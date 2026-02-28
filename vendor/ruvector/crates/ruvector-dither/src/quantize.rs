//! Drop-in quantization helpers that apply dither before rounding.

use crate::DitherSource;

/// Quantize a single value with deterministic dither.
///
/// # Arguments
/// - `x`      – input activation in `[-1.0, 1.0]`
/// - `bits`   – quantizer bit-width (e.g. 3, 5, 7, 8)
/// - `eps`    – dither amplitude in LSB units (0.0 = no dither, 0.5 = half-LSB recommended)
/// - `source` – stateful dither sequence
///
/// Returns the quantized value in `[-1.0, 1.0]`.
///
/// # Example
/// ```
/// use ruvector_dither::{GoldenRatioDither, quantize_dithered};
/// let mut d = GoldenRatioDither::new(0.0);
/// let q = quantize_dithered(0.314, 8, 0.5, &mut d);
/// assert!(q >= -1.0 && q <= 1.0);
/// ```
#[inline]
pub fn quantize_dithered(x: f32, bits: u32, eps: f32, source: &mut impl DitherSource) -> f32 {
    assert!(bits >= 2 && bits <= 31, "bits must be in [2, 31]");
    let qmax = ((1u32 << (bits - 1)) - 1) as f32;
    let lsb = 1.0 / qmax;
    let dither = source.next(eps * lsb);
    let shifted = (x + dither) * qmax;
    let rounded = shifted.round().clamp(-qmax, qmax);
    rounded / qmax
}

/// Quantize a slice in-place with deterministic dither.
///
/// Each element gets an independent dither sample from `source`.
///
/// # Example
/// ```
/// use ruvector_dither::{GoldenRatioDither, quantize_slice_dithered};
/// let mut vals = vec![0.1_f32, 0.5, -0.3, 0.9, -0.8];
/// let mut d = GoldenRatioDither::new(0.0);
/// quantize_slice_dithered(&mut vals, 5, 0.5, &mut d);
/// for &v in &vals {
///     assert!(v >= -1.0 && v <= 1.0);
/// }
/// ```
pub fn quantize_slice_dithered(
    xs: &mut [f32],
    bits: u32,
    eps: f32,
    source: &mut impl DitherSource,
) {
    assert!(bits >= 2 && bits <= 31, "bits must be in [2, 31]");
    let qmax = ((1u32 << (bits - 1)) - 1) as f32;
    let lsb = 1.0 / qmax;
    for x in xs.iter_mut() {
        let dither = source.next(eps * lsb);
        let shifted = (*x + dither) * qmax;
        *x = shifted.round().clamp(-qmax, qmax) / qmax;
    }
}

/// Quantize to a raw integer code (signed, in `[-(2^(bits-1)), 2^(bits-1)-1]`).
///
/// Useful when you need the integer representation rather than a re-scaled float.
#[inline]
pub fn quantize_to_code(x: f32, bits: u32, eps: f32, source: &mut impl DitherSource) -> i32 {
    assert!(bits >= 2 && bits <= 31, "bits must be in [2, 31]");
    let qmax = ((1u32 << (bits - 1)) - 1) as f32;
    let lsb = 1.0 / qmax;
    let dither = source.next(eps * lsb);
    ((x + dither) * qmax).round().clamp(-qmax, qmax) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GoldenRatioDither, PiDither};

    #[test]
    fn output_in_unit_range() {
        let mut d = GoldenRatioDither::new(0.0);
        for bits in [3u32, 5, 7, 8] {
            for &x in &[-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
                let q = quantize_dithered(x, bits, 0.5, &mut d);
                assert!(q >= -1.0 && q <= 1.0, "bits={bits}, x={x}, q={q}");
            }
        }
    }

    #[test]
    fn dither_reduces_idle_tones() {
        // A constant signal at exactly 0.5 * LSB without dither quantizes
        // to the same code every time (idle tone).  With dither the code
        // alternates, so the variance of codes should be > 0.
        let bits = 5u32;
        let qmax = ((1u32 << (bits - 1)) - 1) as f32;
        let lsb = 1.0 / qmax;
        let x = 0.5 * lsb; // exactly half an LSB

        let mut codes_with: Vec<i32> = Vec::with_capacity(256);
        let mut d = GoldenRatioDither::new(0.0);
        for _ in 0..256 {
            codes_with.push(quantize_to_code(x, bits, 0.5, &mut d));
        }
        let unique: std::collections::HashSet<i32> = codes_with.iter().copied().collect();
        assert!(
            unique.len() > 1,
            "dithered signal must produce >1 unique code"
        );
    }

    #[test]
    fn slice_quantize_in_bounds() {
        let mut vals: Vec<f32> = (-50..=50).map(|i| i as f32 * 0.02).collect();
        let mut pi = PiDither::new(0);
        quantize_slice_dithered(&mut vals, 7, 0.5, &mut pi);
        for v in vals {
            assert!(v >= -1.0 && v <= 1.0, "out of range: {v}");
        }
    }

    #[test]
    fn deterministic_with_same_seed() {
        let input = vec![0.1_f32, 0.4, -0.7, 0.9];
        let quantize = |input: &[f32]| {
            let mut buf = input.to_vec();
            let mut d = GoldenRatioDither::new(0.5);
            quantize_slice_dithered(&mut buf, 8, 0.5, &mut d);
            buf
        };
        assert_eq!(quantize(&input), quantize(&input));
    }
}
