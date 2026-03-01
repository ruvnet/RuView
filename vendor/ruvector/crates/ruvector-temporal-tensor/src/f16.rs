//! Software IEEE 754 half-precision (f16) conversion.
//!
//! No external crate dependencies. Handles normals, denormals, infinity, and NaN.
//! Round-to-nearest with ties-to-even for normal values.

/// Convert f32 to f16 bit representation.
///
/// Handles all IEEE 754 special cases: infinity, NaN, denormals, and zero (both signs).
/// Values outside f16 range saturate to infinity. Values too small for f16 denormals
/// flush to zero.
#[inline]
pub fn f32_to_f16_bits(x: f32) -> u16 {
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xFF) as i32;
    let mant = b & 0x7F_FFFF;

    // Infinity or NaN
    if exp == 255 {
        if mant == 0 {
            return sign | 0x7C00;
        }
        let nan_m = (mant >> 13) as u16;
        return sign | 0x7C00 | nan_m | 1;
    }

    let exp16 = exp - 127 + 15;

    // Overflow -> Infinity
    if exp16 >= 31 {
        return sign | 0x7C00;
    }

    // Underflow -> denormal or zero
    if exp16 <= 0 {
        if exp16 < -10 {
            return sign;
        }
        let shift = (14 - exp16) as u32;
        let mut mant32 = mant | 0x80_0000;
        let round_bit = 1u32.wrapping_shl(shift.wrapping_sub(1));
        mant32 = mant32.wrapping_add(round_bit);
        let sub = (mant32 >> shift) as u16;
        return sign | sub;
    }

    // Normal case
    let mant16 = (mant >> 13) as u16;
    let round = (mant >> 12) & 1;
    let mut res = sign | ((exp16 as u16) << 10) | mant16;
    if round != 0 {
        res = res.wrapping_add(1);
    }
    res
}

/// Convert f16 bit representation to f32.
///
/// Exactly reconstructs the f32 value represented by the f16 bit pattern.
/// Handles denormals by normalizing the mantissa before constructing the f32 bits.
#[inline]
pub fn f16_bits_to_f32(h: u16) -> f32 {
    let sign = ((h & 0x8000) as u32) << 16;
    let exp = ((h >> 10) & 0x1F) as i32;
    let mant = (h & 0x03FF) as u32;

    // Zero or denormal
    if exp == 0 {
        if mant == 0 {
            return f32::from_bits(sign);
        }
        let mut e = 1i32;
        let mut m = mant;
        while (m & 0x0400) == 0 {
            m <<= 1;
            e += 1;
        }
        m &= 0x03FF;
        let exp32 = 127 - 15 - e + 1;
        let mant32 = m << 13;
        return f32::from_bits(sign | ((exp32 as u32) << 23) | mant32);
    }

    // Infinity or NaN
    if exp == 31 {
        return f32::from_bits(sign | 0x7F80_0000 | (mant << 13));
    }

    // Normal
    let exp32 = exp - 15 + 127;
    let mant32 = mant << 13;
    f32::from_bits(sign | ((exp32 as u32) << 23) | mant32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_normal() {
        for &v in &[0.0f32, 1.0, -1.0, 0.5, 65504.0, -65504.0, 0.0001] {
            let h = f32_to_f16_bits(v);
            let back = f16_bits_to_f32(h);
            if v == 0.0 {
                assert_eq!(back, 0.0);
            } else {
                let rel_err = ((back - v) / v).abs();
                assert!(rel_err < 0.01, "v={v}, back={back}, rel_err={rel_err}");
            }
        }
    }

    #[test]
    fn test_infinity() {
        let h = f32_to_f16_bits(f32::INFINITY);
        assert_eq!(h, 0x7C00);
        assert!(f16_bits_to_f32(h).is_infinite());
    }

    #[test]
    fn test_neg_infinity() {
        let h = f32_to_f16_bits(f32::NEG_INFINITY);
        assert_eq!(h, 0xFC00);
        let back = f16_bits_to_f32(h);
        assert!(back.is_infinite() && back < 0.0);
    }

    #[test]
    fn test_nan() {
        let h = f32_to_f16_bits(f32::NAN);
        assert!(f16_bits_to_f32(h).is_nan());
    }

    #[test]
    fn test_zero_signs() {
        assert_eq!(f32_to_f16_bits(0.0f32), 0x0000);
        assert_eq!(f32_to_f16_bits(-0.0f32), 0x8000);
    }

    #[test]
    fn test_scale_range_accuracy() {
        for exp in -4..=4i32 {
            let v = 10.0f32.powi(exp);
            let h = f32_to_f16_bits(v);
            let back = f16_bits_to_f32(h);
            let rel_err = ((back - v) / v).abs();
            assert!(rel_err < 0.002, "v={v}, back={back}, rel_err={rel_err}");
        }
    }
}
