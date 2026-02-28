//! Quantization subsystem
//!
//! Explicit, reproducible quantization for weights and activations.

pub mod calib;
pub mod lut;
pub mod qformat;

pub use calib::{calibrate_model, CalibrationData};
pub use lut::{exp_lut, log_lut, softmax_lut};
pub use qformat::{dequantize_i16, dequantize_i8, quantize_i16, quantize_i8};

use crate::types::QuantSpec;

/// Fixed-point Q15 format (1.15)
/// Range: [-1.0, 1.0 - 2^-15]
/// Resolution: 2^-15 ≈ 3.05e-5
pub type Q15 = i16;

/// Fixed-point Q16.16 format
/// Range: [-32768.0, 32767.999...]
/// Resolution: 2^-16 ≈ 1.53e-5
pub type Q16_16 = i32;

/// Convert f32 to Q15
#[inline]
pub fn f32_to_q15(x: f32) -> Q15 {
    (x.clamp(-1.0, 1.0 - f32::EPSILON) * 32768.0) as Q15
}

/// Convert Q15 to f32
#[inline]
pub fn q15_to_f32(x: Q15) -> f32 {
    x as f32 / 32768.0
}

/// Convert f32 to Q16.16
#[inline]
pub fn f32_to_q16_16(x: f32) -> Q16_16 {
    (x * 65536.0) as Q16_16
}

/// Convert Q16.16 to f32
#[inline]
pub fn q16_16_to_f32(x: Q16_16) -> f32 {
    x as f32 / 65536.0
}

/// Fixed-point multiplication Q15 * Q15 -> Q15
#[inline]
pub fn q15_mul(a: Q15, b: Q15) -> Q15 {
    // Multiply with proper rounding
    let product = (a as i32 * b as i32 + 0x4000) >> 15;
    product.clamp(i16::MIN as i32, i16::MAX as i32) as Q15
}

/// Fixed-point dot product with accumulator
/// Note: For very large vectors (>65536 elements), use q15_dot_saturating
#[inline]
pub fn q15_dot(a: &[Q15], b: &[Q15]) -> i32 {
    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| x as i32 * y as i32)
        .sum()
}

/// Saturating fixed-point dot product (prevents overflow for large vectors)
#[inline]
pub fn q15_dot_saturating(a: &[Q15], b: &[Q15]) -> i32 {
    a.iter().zip(b.iter()).fold(0i32, |acc, (&x, &y)| {
        acc.saturating_add((x as i32).saturating_mul(y as i32))
    })
}

/// Fixed-point dot product normalized to Q15
#[inline]
pub fn q15_dot_normalized(a: &[Q15], b: &[Q15], shift: u8) -> Q15 {
    let sum = q15_dot(a, b);
    let shifted = (sum + (1 << (shift - 1))) >> shift;
    shifted.clamp(i16::MIN as i32, i16::MAX as i32) as Q15
}

/// Quantization context for a layer
#[derive(Debug, Clone)]
pub struct QuantContext {
    /// Weight quantization spec
    pub weight_spec: QuantSpec,
    /// Input scale (Q16.16)
    pub input_scale: Q16_16,
    /// Output scale (Q16.16)
    pub output_scale: Q16_16,
    /// Accumulator bit width
    pub acc_bits: u8,
    /// Right shift for normalization
    pub norm_shift: u8,
}

impl QuantContext {
    /// Create from QuantSpec
    pub fn from_spec(spec: &QuantSpec) -> Self {
        Self {
            weight_spec: *spec,
            input_scale: spec.scale_q,
            output_scale: spec.scale_q,
            acc_bits: 32,
            norm_shift: 15,
        }
    }

    /// Compute the required accumulator bits to avoid overflow
    pub fn required_acc_bits(input_bits: u8, weight_bits: u8, vector_len: usize) -> u8 {
        // Each multiply produces input_bits + weight_bits
        // Sum of vector_len terms adds log2(vector_len) bits
        let product_bits = input_bits + weight_bits;
        let sum_bits = (vector_len as f64).log2().ceil() as u8;
        product_bits + sum_bits + 1 // +1 for sign
    }
}

/// Packing utilities for sub-byte quantization
pub mod packing {
    /// Pack two 4-bit values into one byte
    #[inline]
    pub fn pack_int4(a: i8, b: i8) -> u8 {
        ((a & 0x0F) as u8) | (((b & 0x0F) as u8) << 4)
    }

    /// Unpack byte into two 4-bit values
    #[inline]
    pub fn unpack_int4(packed: u8) -> (i8, i8) {
        let a = (packed & 0x0F) as i8;
        let a = if a & 0x08 != 0 { a | !0x0F } else { a }; // Sign extend
        let b = ((packed >> 4) & 0x0F) as i8;
        let b = if b & 0x08 != 0 { b | !0x0F } else { b };
        (a, b)
    }

    /// Pack four 2-bit values into one byte
    #[inline]
    pub fn pack_int2(a: i8, b: i8, c: i8, d: i8) -> u8 {
        ((a & 0x03) as u8)
            | (((b & 0x03) as u8) << 2)
            | (((c & 0x03) as u8) << 4)
            | (((d & 0x03) as u8) << 6)
    }

    /// Unpack byte into four 2-bit values
    #[inline]
    pub fn unpack_int2(packed: u8) -> (i8, i8, i8, i8) {
        let a = (packed & 0x03) as i8;
        let a = if a & 0x02 != 0 { a | !0x03 } else { a };
        let b = ((packed >> 2) & 0x03) as i8;
        let b = if b & 0x02 != 0 { b | !0x03 } else { b };
        let c = ((packed >> 4) & 0x03) as i8;
        let c = if c & 0x02 != 0 { c | !0x03 } else { c };
        let d = ((packed >> 6) & 0x03) as i8;
        let d = if d & 0x02 != 0 { d | !0x03 } else { d };
        (a, b, c, d)
    }

    /// Pack eight 1-bit values into one byte
    #[inline]
    pub fn pack_binary(bits: &[bool; 8]) -> u8 {
        bits.iter()
            .enumerate()
            .fold(0u8, |acc, (i, &b)| acc | ((b as u8) << i))
    }

    /// Unpack byte into eight 1-bit values
    #[inline]
    pub fn unpack_binary(packed: u8) -> [bool; 8] {
        [
            packed & 0x01 != 0,
            packed & 0x02 != 0,
            packed & 0x04 != 0,
            packed & 0x08 != 0,
            packed & 0x10 != 0,
            packed & 0x20 != 0,
            packed & 0x40 != 0,
            packed & 0x80 != 0,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_q15_conversion() {
        assert_eq!(f32_to_q15(0.0), 0);
        assert_eq!(f32_to_q15(0.5), 16384);
        assert_eq!(f32_to_q15(-0.5), -16384);

        let x = 0.123f32;
        let q = f32_to_q15(x);
        let back = q15_to_f32(q);
        assert!((x - back).abs() < 0.001);
    }

    #[test]
    fn test_q15_mul() {
        let a = f32_to_q15(0.5);
        let b = f32_to_q15(0.5);
        let c = q15_mul(a, b);
        let result = q15_to_f32(c);
        assert!((result - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_packing_int4() {
        let (a, b) = (5i8, -3i8);
        let packed = packing::pack_int4(a, b);
        let (ua, ub) = packing::unpack_int4(packed);
        assert_eq!(a, ua);
        assert_eq!(b, ub);
    }

    #[test]
    fn test_packing_int2() {
        let (a, b, c, d) = (1i8, -1i8, 0i8, -2i8);
        let packed = packing::pack_int2(a, b, c, d);
        let (ua, ub, uc, ud) = packing::unpack_int2(packed);
        assert_eq!(a, ua);
        assert_eq!(b, ub);
        assert_eq!(c, uc);
        // -2 in 2-bit is 10 binary, which unpacks to -2 (sign extended)
        assert_eq!(-2i8, ud);
    }

    #[test]
    fn test_packing_binary() {
        let bits = [true, false, true, true, false, false, true, false];
        let packed = packing::pack_binary(&bits);
        let unpacked = packing::unpack_binary(packed);
        assert_eq!(bits, unpacked);
    }
}
