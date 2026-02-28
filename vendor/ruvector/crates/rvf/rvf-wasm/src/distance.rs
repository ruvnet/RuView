//! Distance functions for vector similarity computation.
//!
//! Scalar fallbacks for all metrics. WASM v128 SIMD would be added
//! as a future optimization when targeting wasm32 with simd128 feature.

/// Convert a 16-bit IEEE 754 half-precision value to f32.
#[inline]
fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits >> 15) & 1) as u32;
    let exp = ((bits >> 10) & 0x1F) as u32;
    let mantissa = (bits & 0x03FF) as u32;

    if exp == 0 {
        if mantissa == 0 {
            return f32::from_bits(sign << 31);
        }
        // Subnormal: normalize
        let mut m = mantissa;
        let mut e: i32 = -14;
        while m & 0x0400 == 0 {
            m <<= 1;
            e -= 1;
        }
        m &= 0x03FF;
        let f32_exp = ((e + 127) as u32) & 0xFF;
        return f32::from_bits((sign << 31) | (f32_exp << 23) | (m << 13));
    }

    if exp == 0x1F {
        let f32_mantissa = mantissa << 13;
        return f32::from_bits((sign << 31) | (0xFF << 23) | f32_mantissa);
    }

    let f32_exp = (exp as i32 - 15 + 127) as u32;
    f32::from_bits((sign << 31) | (f32_exp << 23) | (mantissa << 13))
}

/// Read a u16 from a byte pointer at the given index (little-endian).
#[inline]
unsafe fn read_u16(ptr: *const u8, idx: usize) -> u16 {
    let p = ptr.add(idx * 2);
    u16::from_le_bytes([*p, *p.add(1)])
}

/// L2 (squared Euclidean) distance between two fp16 vectors.
pub fn l2_fp16(a_ptr: *const u8, b_ptr: *const u8, dim: usize) -> f32 {
    let mut sum: f32 = 0.0;
    for i in 0..dim {
        let a = f16_to_f32(unsafe { read_u16(a_ptr, i) });
        let b = f16_to_f32(unsafe { read_u16(b_ptr, i) });
        let diff = a - b;
        sum += diff * diff;
    }
    sum
}

/// Inner product distance between two fp16 vectors.
/// Returns negative inner product (so smaller = more similar).
pub fn ip_fp16(a_ptr: *const u8, b_ptr: *const u8, dim: usize) -> f32 {
    let mut sum: f32 = 0.0;
    for i in 0..dim {
        let a = f16_to_f32(unsafe { read_u16(a_ptr, i) });
        let b = f16_to_f32(unsafe { read_u16(b_ptr, i) });
        sum += a * b;
    }
    -sum
}

/// Cosine distance between two fp16 vectors.
/// Returns 1.0 - cosine_similarity.
pub fn cosine_fp16(a_ptr: *const u8, b_ptr: *const u8, dim: usize) -> f32 {
    let mut dot: f32 = 0.0;
    let mut norm_a: f32 = 0.0;
    let mut norm_b: f32 = 0.0;
    for i in 0..dim {
        let a = f16_to_f32(unsafe { read_u16(a_ptr, i) });
        let b = f16_to_f32(unsafe { read_u16(b_ptr, i) });
        dot += a * b;
        norm_a += a * a;
        norm_b += b * b;
    }
    let denom = sqrt_approx(norm_a) * sqrt_approx(norm_b);
    if denom < 1e-10 {
        return 1.0;
    }
    1.0 - (dot / denom)
}

/// Hamming distance between two byte arrays.
/// Counts the number of differing bits.
pub fn hamming(a_ptr: *const u8, b_ptr: *const u8, byte_len: usize) -> f32 {
    let mut count: u32 = 0;
    for i in 0..byte_len {
        let xor = unsafe { *a_ptr.add(i) ^ *b_ptr.add(i) };
        count += xor.count_ones();
    }
    count as f32
}

/// L2 (squared Euclidean) distance between two i8 vectors.
pub fn l2_i8(a_ptr: *const u8, b_ptr: *const u8, dim: usize) -> f32 {
    let mut sum: f32 = 0.0;
    for i in 0..dim {
        let a = unsafe { *a_ptr.add(i) } as i8 as f32;
        let b = unsafe { *b_ptr.add(i) } as i8 as f32;
        let diff = a - b;
        sum += diff * diff;
    }
    sum
}

/// Fast approximate square root.
#[inline]
fn sqrt_approx(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut bits = x.to_bits();
    bits = 0x1FBD_1DF5 + (bits >> 1);
    let mut y = f32::from_bits(bits);
    y = 0.5 * (y + x / y);
    y = 0.5 * (y + x / y);
    y
}
