use super::k_quants::{
    BlockQ2K, BlockQ3K, BlockQ4K, BlockQ4_0, BlockQ5K, BlockQ6K, BlockQ8K, BlockQ8_0, QK8_0, QK_K,
};
use byteorder::{ByteOrder, LittleEndian};
use half::f16;

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

#[inline(always)]
pub(crate) unsafe fn sum_i16_pairs_float(x: __m256i) -> __m256 {
    let ones = _mm256_set1_epi16(1);
    let summed_pairs = _mm256_madd_epi16(ones, x);
    _mm256_cvtepi32_ps(summed_pairs)
}

#[inline(always)]
pub(crate) unsafe fn mul_sum_us8_pairs_float(ax: __m256i, sy: __m256i) -> __m256 {
    let dot = _mm256_maddubs_epi16(ax, sy);
    sum_i16_pairs_float(dot)
}

#[inline(always)]
pub(crate) unsafe fn hsum_float_8(x: __m256) -> f32 {
    let res = _mm256_extractf128_ps(x, 1);
    let res = _mm_add_ps(res, _mm256_castps256_ps128(x));
    let res = _mm_add_ps(res, _mm_movehl_ps(res, res));
    let res = _mm_add_ss(res, _mm_movehdup_ps(res));
    _mm_cvtss_f32(res)
}

#[inline(always)]
pub(crate) unsafe fn bytes_from_nibbles_32(rsi: *const u8) -> __m256i {
    let tmp = _mm_loadu_si128(rsi as *const __m128i);
    let bytes = _mm256_insertf128_si256::<1>(_mm256_castsi128_si256(tmp), _mm_srli_epi16(tmp, 4));
    let low_mask = _mm256_set1_epi8(0xF);
    _mm256_and_si256(low_mask, bytes)
}

#[inline(always)]
pub(crate) unsafe fn mul_sum_i8_pairs_float(x: __m256i, y: __m256i) -> __m256 {
    let ax = _mm256_sign_epi8(x, x);
    let sy = _mm256_sign_epi8(y, x);
    mul_sum_us8_pairs_float(ax, sy)
}

#[inline(always)]
pub(crate) fn vec_dot_q4_0_q8_0(n: usize, xs: &[BlockQ4_0], ys: &[BlockQ8_0]) -> f32 {
    debug_assert!(
        n.is_multiple_of(QK8_0),
        "vec_dot_q4_0_q8_0: {n} is not divisible by {QK8_0}"
    );
    unsafe {
        let mut acc = _mm256_setzero_ps();
        for (x, y) in xs.iter().zip(ys.iter()) {
            let d = _mm256_set1_ps(f16::to_f32(x.d) * f16::to_f32(y.d));
            let bx = bytes_from_nibbles_32(x.qs.as_ptr());
            let off = _mm256_set1_epi8(8);
            let bx = _mm256_sub_epi8(bx, off);
            let by = _mm256_loadu_si256(y.qs.as_ptr() as *const __m256i);
            let q = mul_sum_i8_pairs_float(bx, by);
            acc = _mm256_fmadd_ps(d, q, acc);
        }
        hsum_float_8(acc)
    }
}

#[inline(always)]
pub(crate) fn vec_dot_q8_0_q8_0(n: usize, xs: &[BlockQ8_0], ys: &[BlockQ8_0]) -> f32 {
    debug_assert!(
        n.is_multiple_of(QK8_0),
        "vec_dot_q8_0_q8_0: {n} is not divisible by {QK8_0}"
    );
    unsafe {
        let mut acc = _mm256_setzero_ps();
        for (x, y) in xs.iter().zip(ys.iter()) {
            let d = _mm256_set1_ps(f16::to_f32(x.d) * f16::to_f32(y.d));
            let bx = _mm256_loadu_si256(x.qs.as_ptr() as *const __m256i);
            let by = _mm256_loadu_si256(y.qs.as_ptr() as *const __m256i);
            let q = mul_sum_i8_pairs_float(bx, by);
            acc = _mm256_fmadd_ps(d, q, acc);
        }
        hsum_float_8(acc)
    }
}

#[inline(always)]
unsafe fn get_scale_shuffle(i: usize) -> __m128i {
    const K_SHUFFLE: [u8; 128] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3,
        3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 7, 7, 7, 7,
        7, 7, 7, 7, 8, 8, 8, 8, 8, 8, 8, 8, 9, 9, 9, 9, 9, 9, 9, 9, 10, 10, 10, 10, 10, 10, 10, 10,
        11, 11, 11, 11, 11, 11, 11, 11, 12, 12, 12, 12, 12, 12, 12, 12, 13, 13, 13, 13, 13, 13, 13,
        13, 14, 14, 14, 14, 14, 14, 14, 14, 15, 15, 15, 15, 15, 15, 15, 15,
    ];
    _mm_loadu_si128((K_SHUFFLE.as_ptr() as *const __m128i).add(i))
}

#[inline(always)]
unsafe fn get_scale_shuffle_k4(i: usize) -> __m256i {
    const K_SHUFFLE: [u8; 256] = [
        0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1,
        0, 1, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3,
        2, 3, 2, 3, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5,
        4, 5, 4, 5, 4, 5, 6, 7, 6, 7, 6, 7, 6, 7, 6, 7, 6, 7, 6, 7, 6, 7, 6, 7, 6, 7, 6, 7, 6, 7,
        6, 7, 6, 7, 6, 7, 6, 7, 8, 9, 8, 9, 8, 9, 8, 9, 8, 9, 8, 9, 8, 9, 8, 9, 8, 9, 8, 9, 8, 9,
        8, 9, 8, 9, 8, 9, 8, 9, 8, 9, 10, 11, 10, 11, 10, 11, 10, 11, 10, 11, 10, 11, 10, 11, 10,
        11, 10, 11, 10, 11, 10, 11, 10, 11, 10, 11, 10, 11, 10, 11, 10, 11, 12, 13, 12, 13, 12, 13,
        12, 13, 12, 13, 12, 13, 12, 13, 12, 13, 12, 13, 12, 13, 12, 13, 12, 13, 12, 13, 12, 13, 12,
        13, 12, 13, 14, 15, 14, 15, 14, 15, 14, 15, 14, 15, 14, 15, 14, 15, 14, 15, 14, 15, 14, 15,
        14, 15, 14, 15, 14, 15, 14, 15, 14, 15, 14, 15,
    ];
    _mm256_loadu_si256((K_SHUFFLE.as_ptr() as *const __m256i).add(i))
}

#[inline(always)]
unsafe fn get_scale_shuffle_q3k(i: usize) -> __m256i {
    const K_SHUFFLE: [u8; 128] = [
        0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3, 2, 3,
        2, 3, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 6, 7, 6, 7, 6, 7, 6, 7, 6, 7, 6, 7,
        6, 7, 6, 7, 8, 9, 8, 9, 8, 9, 8, 9, 8, 9, 8, 9, 8, 9, 8, 9, 10, 11, 10, 11, 10, 11, 10, 11,
        10, 11, 10, 11, 10, 11, 10, 11, 12, 13, 12, 13, 12, 13, 12, 13, 12, 13, 12, 13, 12, 13, 12,
        13, 14, 15, 14, 15, 14, 15, 14, 15, 14, 15, 14, 15, 14, 15, 14, 15,
    ];
    _mm256_loadu_si256((K_SHUFFLE.as_ptr() as *const __m256i).add(i))
}

#[inline(always)]
pub(crate) fn vec_dot_q6k_q8k(n: usize, xs: &[BlockQ6K], ys: &[BlockQ8K]) -> f32 {
    debug_assert!(
        n.is_multiple_of(QK_K),
        "vec_dot_q6k_8k: {n} is not divisible by {QK_K}"
    );

    unsafe {
        let m4 = _mm256_set1_epi8(0xF);
        let m2 = _mm256_set1_epi8(3);
        let m32s = _mm256_set1_epi8(32);
        let mut acc = _mm256_setzero_ps();
        for (x, y) in xs.iter().zip(ys.iter()) {
            let d = y.d * x.d.to_f32();
            let mut q4 = x.ql.as_ptr();
            let mut qh = x.qh.as_ptr();
            let mut q8 = y.qs.as_ptr();

            let scales = _mm_loadu_si128(x.scales.as_ptr() as *const __m128i);
            let mut sumi = _mm256_setzero_si256();

            for j in 0..QK_K / 128 {
                let is = j * 4;
                let scale_0 = _mm_shuffle_epi8(scales, get_scale_shuffle(is));
                let scale_1 = _mm_shuffle_epi8(scales, get_scale_shuffle(is + 1));
                let scale_2 = _mm_shuffle_epi8(scales, get_scale_shuffle(is + 2));
                let scale_3 = _mm_shuffle_epi8(scales, get_scale_shuffle(is + 3));

                let q4bits1 = _mm256_loadu_si256(q4 as *const __m256i);
                q4 = q4.add(32);
                let q4bits2 = _mm256_loadu_si256(q4 as *const __m256i);
                q4 = q4.add(32);
                let q4bits_h = _mm256_loadu_si256(qh as *const __m256i);
                qh = qh.add(32);

                let q4h_0 = _mm256_slli_epi16(_mm256_and_si256(q4bits_h, m2), 4);
                let q4h_1 =
                    _mm256_slli_epi16(_mm256_and_si256(_mm256_srli_epi16(q4bits_h, 2), m2), 4);
                let q4h_2 =
                    _mm256_slli_epi16(_mm256_and_si256(_mm256_srli_epi16(q4bits_h, 4), m2), 4);
                let q4h_3 =
                    _mm256_slli_epi16(_mm256_and_si256(_mm256_srli_epi16(q4bits_h, 6), m2), 4);

                let q4_0 = _mm256_or_si256(_mm256_and_si256(q4bits1, m4), q4h_0);
                let q4_1 = _mm256_or_si256(_mm256_and_si256(q4bits2, m4), q4h_1);
                let q4_2 =
                    _mm256_or_si256(_mm256_and_si256(_mm256_srli_epi16(q4bits1, 4), m4), q4h_2);
                let q4_3 =
                    _mm256_or_si256(_mm256_and_si256(_mm256_srli_epi16(q4bits2, 4), m4), q4h_3);

                let q8_0 = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);
                let q8_1 = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);
                let q8_2 = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);
                let q8_3 = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);

                let q8s_0 = _mm256_maddubs_epi16(m32s, q8_0);
                let q8s_1 = _mm256_maddubs_epi16(m32s, q8_1);
                let q8s_2 = _mm256_maddubs_epi16(m32s, q8_2);
                let q8s_3 = _mm256_maddubs_epi16(m32s, q8_3);

                let p16_0 = _mm256_maddubs_epi16(q4_0, q8_0);
                let p16_1 = _mm256_maddubs_epi16(q4_1, q8_1);
                let p16_2 = _mm256_maddubs_epi16(q4_2, q8_2);
                let p16_3 = _mm256_maddubs_epi16(q4_3, q8_3);

                let p16_0 = _mm256_sub_epi16(p16_0, q8s_0);
                let p16_1 = _mm256_sub_epi16(p16_1, q8s_1);
                let p16_2 = _mm256_sub_epi16(p16_2, q8s_2);
                let p16_3 = _mm256_sub_epi16(p16_3, q8s_3);

                let p16_0 = _mm256_madd_epi16(_mm256_cvtepi8_epi16(scale_0), p16_0);
                let p16_1 = _mm256_madd_epi16(_mm256_cvtepi8_epi16(scale_1), p16_1);
                let p16_2 = _mm256_madd_epi16(_mm256_cvtepi8_epi16(scale_2), p16_2);
                let p16_3 = _mm256_madd_epi16(_mm256_cvtepi8_epi16(scale_3), p16_3);

                sumi = _mm256_add_epi32(sumi, _mm256_add_epi32(p16_0, p16_1));
                sumi = _mm256_add_epi32(sumi, _mm256_add_epi32(p16_2, p16_3));
            }
            acc = _mm256_fmadd_ps(_mm256_broadcast_ss(&d), _mm256_cvtepi32_ps(sumi), acc);
        }
        hsum_float_8(acc)
    }
}

#[inline(always)]
unsafe fn mm256_set_m128i(a: __m128i, b: __m128i) -> __m256i {
    _mm256_insertf128_si256(_mm256_castsi128_si256(b), a, 1)
}

#[inline(always)]
pub(crate) fn vec_dot_q2k_q8k(n: usize, xs: &[BlockQ2K], ys: &[BlockQ8K]) -> f32 {
    debug_assert!(
        n.is_multiple_of(QK_K),
        "vec_dot_q2k_q8k: {n} is not divisible by {QK_K}"
    );

    unsafe {
        let m3 = _mm256_set1_epi8(3);
        let m4 = _mm_set1_epi8(0xF);

        let mut acc = _mm256_setzero_ps();

        for (x, y) in xs.iter().zip(ys.iter()) {
            let d = y.d * x.d.to_f32();
            let dmin = -y.d * x.dmin.to_f32();

            let mut q2 = x.qs.as_ptr();
            let mut q8 = y.qs.as_ptr();

            let mins_and_scales = _mm_loadu_si128(x.scales.as_ptr() as *const __m128i);
            let scales8 = _mm_and_si128(mins_and_scales, m4);
            let mins8 = _mm_and_si128(_mm_srli_epi16(mins_and_scales, 4), m4);
            let mins = _mm256_cvtepi8_epi16(mins8);
            let prod =
                _mm256_madd_epi16(mins, _mm256_loadu_si256(y.bsums.as_ptr() as *const __m256i));

            acc = _mm256_fmadd_ps(_mm256_broadcast_ss(&dmin), _mm256_cvtepi32_ps(prod), acc);

            let all_scales = _mm256_cvtepi8_epi16(scales8);
            let l_scales = _mm256_extracti128_si256(all_scales, 0);
            let h_scales = _mm256_extracti128_si256(all_scales, 1);
            let scales = [
                mm256_set_m128i(l_scales, l_scales),
                mm256_set_m128i(h_scales, h_scales),
            ];

            let mut sumi = _mm256_setzero_si256();

            for scale in scales {
                let q2bits = _mm256_loadu_si256(q2 as *const __m256i);
                q2 = q2.add(32);

                let q8_0 = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);
                let q8_1 = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);
                let q8_2 = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);
                let q8_3 = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);

                let q2_0 = _mm256_and_si256(q2bits, m3);
                let q2_1 = _mm256_and_si256(_mm256_srli_epi16(q2bits, 2), m3);
                let q2_2 = _mm256_and_si256(_mm256_srli_epi16(q2bits, 4), m3);
                let q2_3 = _mm256_and_si256(_mm256_srli_epi16(q2bits, 6), m3);

                let p0 = _mm256_maddubs_epi16(q2_0, q8_0);
                let p1 = _mm256_maddubs_epi16(q2_1, q8_1);
                let p2 = _mm256_maddubs_epi16(q2_2, q8_2);
                let p3 = _mm256_maddubs_epi16(q2_3, q8_3);

                let p0 =
                    _mm256_madd_epi16(_mm256_shuffle_epi8(scale, get_scale_shuffle_q3k(0)), p0);
                let p1 =
                    _mm256_madd_epi16(_mm256_shuffle_epi8(scale, get_scale_shuffle_q3k(1)), p1);
                let p2 =
                    _mm256_madd_epi16(_mm256_shuffle_epi8(scale, get_scale_shuffle_q3k(2)), p2);
                let p3 =
                    _mm256_madd_epi16(_mm256_shuffle_epi8(scale, get_scale_shuffle_q3k(3)), p3);

                let p0 = _mm256_add_epi32(p0, p1);
                let p2 = _mm256_add_epi32(p2, p3);

                sumi = _mm256_add_epi32(sumi, _mm256_add_epi32(p0, p2));
            }
            acc = _mm256_fmadd_ps(_mm256_broadcast_ss(&d), _mm256_cvtepi32_ps(sumi), acc);
        }

        hsum_float_8(acc)
    }
}

#[inline(always)]
pub(crate) fn vec_dot_q3k_q8k(n: usize, xs: &[BlockQ3K], ys: &[BlockQ8K]) -> f32 {
    debug_assert!(
        n.is_multiple_of(QK_K),
        "vec_dot_q3k_q8k: {n} is not divisible by {QK_K}"
    );

    const KMASK1: u32 = 0x03030303;
    const KMASK2: u32 = 0x0f0f0f0f;

    let mut aux = [0u32; 3];

    unsafe {
        let m3 = _mm256_set1_epi8(3);
        let mone = _mm256_set1_epi8(1);
        let m32 = _mm_set1_epi8(32);

        let mut acc = _mm256_setzero_ps();
        for (x, y) in xs.iter().zip(ys.iter()) {
            let d = y.d * x.d.to_f32();

            let mut q3 = x.qs.as_ptr();
            let mut q8 = y.qs.as_ptr();

            LittleEndian::read_u32_into(&x.scales, &mut aux);
            let scales128 = _mm_set_epi32(
                (((aux[1] >> 4) & KMASK2) | (((aux[2] >> 6) & KMASK1) << 4)) as i32,
                (((aux[0] >> 4) & KMASK2) | (((aux[2] >> 4) & KMASK1) << 4)) as i32,
                ((aux[1] & KMASK2) | (((aux[2] >> 2) & KMASK1) << 4)) as i32,
                ((aux[0] & KMASK2) | (((aux[2]) & KMASK1) << 4)) as i32,
            );
            let scales128 = _mm_sub_epi8(scales128, m32);
            let all_scales = _mm256_cvtepi8_epi16(scales128);
            let l_scales = _mm256_extracti128_si256(all_scales, 0);
            let h_scales = _mm256_extracti128_si256(all_scales, 1);
            let scales = [
                mm256_set_m128i(l_scales, l_scales),
                mm256_set_m128i(h_scales, h_scales),
            ];

            // high bit
            let hbits = _mm256_loadu_si256(x.hmask.as_ptr() as *const __m256i);

            let mut sumi = _mm256_setzero_si256();

            for (j, scale) in scales.iter().enumerate() {
                // load low 2 bits
                let q3bits = _mm256_loadu_si256(q3 as *const __m256i);
                q3 = q3.add(32);

                // Prepare low and high bits
                // We hardcode the shifts here to avoid loading them into a separate register
                let q3l_0 = _mm256_and_si256(q3bits, m3);
                let q3h_0 = if j == 0 {
                    _mm256_srli_epi16(_mm256_andnot_si256(hbits, _mm256_slli_epi16(mone, 0)), 0)
                } else {
                    _mm256_srli_epi16(_mm256_andnot_si256(hbits, _mm256_slli_epi16(mone, 4)), 4)
                };
                let q3h_0 = _mm256_slli_epi16(q3h_0, 2);

                let q3l_1 = _mm256_and_si256(_mm256_srli_epi16(q3bits, 2), m3);
                let q3h_1 = if j == 0 {
                    _mm256_srli_epi16(_mm256_andnot_si256(hbits, _mm256_slli_epi16(mone, 1)), 1)
                } else {
                    _mm256_srli_epi16(_mm256_andnot_si256(hbits, _mm256_slli_epi16(mone, 5)), 5)
                };
                let q3h_1 = _mm256_slli_epi16(q3h_1, 2);

                let q3l_2 = _mm256_and_si256(_mm256_srli_epi16(q3bits, 4), m3);
                let q3h_2 = if j == 0 {
                    _mm256_srli_epi16(_mm256_andnot_si256(hbits, _mm256_slli_epi16(mone, 2)), 2)
                } else {
                    _mm256_srli_epi16(_mm256_andnot_si256(hbits, _mm256_slli_epi16(mone, 6)), 6)
                };
                let q3h_2 = _mm256_slli_epi16(q3h_2, 2);

                let q3l_3 = _mm256_and_si256(_mm256_srli_epi16(q3bits, 6), m3);
                let q3h_3 = if j == 0 {
                    _mm256_srli_epi16(_mm256_andnot_si256(hbits, _mm256_slli_epi16(mone, 3)), 3)
                } else {
                    _mm256_srli_epi16(_mm256_andnot_si256(hbits, _mm256_slli_epi16(mone, 7)), 7)
                };
                let q3h_3 = _mm256_slli_epi16(q3h_3, 2);

                // load Q8 quants
                let q8_0 = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);
                let q8_1 = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);
                let q8_2 = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);
                let q8_3 = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);

                // Dot product: we multiply the 2 low bits and 1 high bit part separately, so we
                // can use _mm256_maddubs_epi16, and then subtract. The high bit part has the 2
                // already subtracted (and so, it is zero if the high bit was not set, and 2 if the
                // high bit was set)
                let q8s_0 = _mm256_maddubs_epi16(q3h_0, q8_0);
                let q8s_1 = _mm256_maddubs_epi16(q3h_1, q8_1);
                let q8s_2 = _mm256_maddubs_epi16(q3h_2, q8_2);
                let q8s_3 = _mm256_maddubs_epi16(q3h_3, q8_3);

                let p16_0 = _mm256_maddubs_epi16(q3l_0, q8_0);
                let p16_1 = _mm256_maddubs_epi16(q3l_1, q8_1);
                let p16_2 = _mm256_maddubs_epi16(q3l_2, q8_2);
                let p16_3 = _mm256_maddubs_epi16(q3l_3, q8_3);

                let p16_0 = _mm256_sub_epi16(p16_0, q8s_0);
                let p16_1 = _mm256_sub_epi16(p16_1, q8s_1);
                let p16_2 = _mm256_sub_epi16(p16_2, q8s_2);
                let p16_3 = _mm256_sub_epi16(p16_3, q8s_3);

                // multiply with scales
                let p16_0 =
                    _mm256_madd_epi16(_mm256_shuffle_epi8(*scale, get_scale_shuffle_q3k(0)), p16_0);
                let p16_1 =
                    _mm256_madd_epi16(_mm256_shuffle_epi8(*scale, get_scale_shuffle_q3k(1)), p16_1);
                let p16_2 =
                    _mm256_madd_epi16(_mm256_shuffle_epi8(*scale, get_scale_shuffle_q3k(2)), p16_2);
                let p16_3 =
                    _mm256_madd_epi16(_mm256_shuffle_epi8(*scale, get_scale_shuffle_q3k(3)), p16_3);

                // accumulate
                let p16_0 = _mm256_add_epi32(p16_0, p16_1);
                let p16_2 = _mm256_add_epi32(p16_2, p16_3);
                sumi = _mm256_add_epi32(sumi, _mm256_add_epi32(p16_0, p16_2));
            }

            // multiply with block scale and accumulate
            acc = _mm256_fmadd_ps(_mm256_broadcast_ss(&d), _mm256_cvtepi32_ps(sumi), acc);
        }
        hsum_float_8(acc)
    }
}

#[inline(always)]
pub(crate) fn vec_dot_q4k_q8k(n: usize, xs: &[BlockQ4K], ys: &[BlockQ8K]) -> f32 {
    debug_assert!(
        n.is_multiple_of(QK_K),
        "vec_dot_q4k_q8k: {n} is not divisible by {QK_K}"
    );
    let mut utmp = [0u32; 4];
    const KMASK1: u32 = 0x3f3f3f3f;
    const KMASK2: u32 = 0x0f0f0f0f;
    const KMASK3: u32 = 0x03030303;

    unsafe {
        let m4 = _mm256_set1_epi8(0xF);

        let mut acc = _mm256_setzero_ps();
        let mut acc_m = _mm_setzero_ps();

        for (x, y) in xs.iter().zip(ys.iter()) {
            let d = y.d * x.d.to_f32();
            let dmin = -y.d * x.dmin.to_f32();

            LittleEndian::read_u32_into(&x.scales, &mut utmp[0..3]);

            utmp[3] = ((utmp[2] >> 4) & KMASK2) | (((utmp[1] >> 6) & KMASK3) << 4);
            let uaux = utmp[1] & KMASK1;
            utmp[1] = (utmp[2] & KMASK2) | (((utmp[0] >> 6) & KMASK3) << 4);
            utmp[2] = uaux;
            utmp[0] &= KMASK1;

            let mut q4 = x.qs.as_ptr();
            let mut q8 = y.qs.as_ptr();

            let mins_and_scales = _mm256_cvtepu8_epi16(_mm_set_epi32(
                utmp[3] as i32,
                utmp[2] as i32,
                utmp[1] as i32,
                utmp[0] as i32,
            ));

            let q8sums = _mm256_loadu_si256(y.bsums.as_ptr() as *const __m256i);
            let q8s = _mm_hadd_epi16(
                _mm256_extracti128_si256(q8sums, 0),
                _mm256_extracti128_si256(q8sums, 1),
            );
            let prod = _mm_madd_epi16(_mm256_extracti128_si256(mins_and_scales, 1), q8s);
            acc_m = _mm_fmadd_ps(_mm_set1_ps(dmin), _mm_cvtepi32_ps(prod), acc_m);

            let sc128 = _mm256_extracti128_si256(mins_and_scales, 0);
            let scales = mm256_set_m128i(sc128, sc128);

            let mut sumi = _mm256_setzero_si256();

            for j in 0..QK_K / 64 {
                let scale_l = _mm256_shuffle_epi8(scales, get_scale_shuffle_k4(2 * j));
                let scale_h = _mm256_shuffle_epi8(scales, get_scale_shuffle_k4(2 * j + 1));

                let q4bits = _mm256_loadu_si256(q4 as *const __m256i);
                q4 = q4.add(32);
                let q4l = _mm256_and_si256(q4bits, m4);
                let q4h = _mm256_and_si256(_mm256_srli_epi16(q4bits, 4), m4);

                let q8l = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);
                let p16l = _mm256_maddubs_epi16(q4l, q8l);
                let p16l = _mm256_madd_epi16(scale_l, p16l);
                sumi = _mm256_add_epi32(sumi, p16l);

                let q8h = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);
                let p16h = _mm256_maddubs_epi16(q4h, q8h);
                let p16h = _mm256_madd_epi16(scale_h, p16h);
                sumi = _mm256_add_epi32(sumi, p16h);
            }

            let vd = _mm256_set1_ps(d);
            acc = _mm256_fmadd_ps(vd, _mm256_cvtepi32_ps(sumi), acc);
        }

        let acc_m = _mm_add_ps(acc_m, _mm_movehl_ps(acc_m, acc_m));
        let acc_m = _mm_add_ss(acc_m, _mm_movehdup_ps(acc_m));

        hsum_float_8(acc) + _mm_cvtss_f32(acc_m)
    }
}

#[inline(always)]
pub(crate) fn vec_dot_q5k_q8k(n: usize, xs: &[BlockQ5K], ys: &[BlockQ8K]) -> f32 {
    debug_assert!(
        n.is_multiple_of(QK_K),
        "vec_dot_q5k_q8k: {n} is not divisible by {QK_K}"
    );
    let mut utmp = [0u32; 4];
    const KMASK1: u32 = 0x3f3f3f3f;
    const KMASK2: u32 = 0x0f0f0f0f;
    const KMASK3: u32 = 0x03030303;

    unsafe {
        let m4 = _mm256_set1_epi8(0xF);
        let mzero = _mm_setzero_si128();
        let mone = _mm256_set1_epi8(1);

        let mut acc = _mm256_setzero_ps();
        let mut summs = 0.0;

        for (x, y) in xs.iter().zip(ys.iter()) {
            let d = y.d * x.d.to_f32();
            let dmin = -y.d * x.dmin.to_f32();

            LittleEndian::read_u32_into(&x.scales, &mut utmp[0..3]);

            utmp[3] = ((utmp[2] >> 4) & KMASK2) | (((utmp[1] >> 6) & KMASK3) << 4);
            let uaux = utmp[1] & KMASK1;
            utmp[1] = (utmp[2] & KMASK2) | (((utmp[0] >> 6) & KMASK3) << 4);
            utmp[2] = uaux;
            utmp[0] &= KMASK1;

            let mut q5 = x.qs.as_ptr();
            let mut q8 = y.qs.as_ptr();

            let mins_and_scales = _mm256_cvtepu8_epi16(_mm_set_epi32(
                utmp[3] as i32,
                utmp[2] as i32,
                utmp[1] as i32,
                utmp[0] as i32,
            ));

            let q8sums = _mm256_loadu_si256(y.bsums.as_ptr() as *const __m256i);
            let q8s = _mm_hadd_epi16(
                _mm256_extracti128_si256(q8sums, 0),
                _mm256_extracti128_si256(q8sums, 1),
            );
            let prod = _mm_madd_epi16(_mm256_extracti128_si256(mins_and_scales, 1), q8s);
            let hsum = _mm_hadd_epi32(_mm_hadd_epi32(prod, mzero), mzero);
            summs += dmin * _mm_extract_epi32(hsum, 0) as f32;

            let sc128 = _mm256_extracti128_si256(mins_and_scales, 0);
            let scales = mm256_set_m128i(sc128, sc128);

            let hbits = _mm256_loadu_si256(x.qh.as_ptr() as *const __m256i);
            let mut hmask = mone;

            let mut sumi = _mm256_setzero_si256();

            for j in 0..QK_K / 64 {
                let scale_0 = _mm256_shuffle_epi8(scales, get_scale_shuffle_k4(2 * j));
                let scale_1 = _mm256_shuffle_epi8(scales, get_scale_shuffle_k4(2 * j + 1));

                let q5bits = _mm256_loadu_si256(q5 as *const __m256i);
                q5 = q5.add(32);

                //Similar to q3k we hardcode the shifts here to avoid loading them into a separate register
                let q5l_0 = _mm256_and_si256(q5bits, m4);
                let q5l_0_shift_input = _mm256_and_si256(hbits, hmask);
                let q5l_0_right_shift = match j {
                    0 => _mm256_srli_epi16(q5l_0_shift_input, 0),
                    1 => _mm256_srli_epi16(q5l_0_shift_input, 2),
                    2 => _mm256_srli_epi16(q5l_0_shift_input, 4),
                    3 => _mm256_srli_epi16(q5l_0_shift_input, 6),
                    _ => unreachable!(),
                };
                let q5h_0 = _mm256_slli_epi16(q5l_0_right_shift, 4);
                let q5_0 = _mm256_add_epi8(q5l_0, q5h_0);
                hmask = _mm256_slli_epi16(hmask, 1);

                let q5l_1 = _mm256_and_si256(_mm256_srli_epi16(q5bits, 4), m4);
                let q5l_1_shift_input = _mm256_and_si256(hbits, hmask);
                let q5l_1_right_shift = match j {
                    0 => _mm256_srli_epi16(q5l_1_shift_input, 1),
                    1 => _mm256_srli_epi16(q5l_1_shift_input, 3),
                    2 => _mm256_srli_epi16(q5l_1_shift_input, 5),
                    3 => _mm256_srli_epi16(q5l_1_shift_input, 7),
                    _ => unreachable!(),
                };

                let q5h_1 = _mm256_slli_epi16(q5l_1_right_shift, 4);
                let q5_1 = _mm256_add_epi8(q5l_1, q5h_1);
                hmask = _mm256_slli_epi16(hmask, 1);

                let q8_0 = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);
                let q8_1 = _mm256_loadu_si256(q8 as *const __m256i);
                q8 = q8.add(32);

                let p16_0 = _mm256_maddubs_epi16(q5_0, q8_0);
                let p16_1 = _mm256_maddubs_epi16(q5_1, q8_1);

                let p16_0 = _mm256_madd_epi16(scale_0, p16_0);
                let p16_1 = _mm256_madd_epi16(scale_1, p16_1);

                sumi = _mm256_add_epi32(sumi, _mm256_add_epi32(p16_0, p16_1));
            }
            let vd = _mm256_set1_ps(d);
            acc = _mm256_fmadd_ps(vd, _mm256_cvtepi32_ps(sumi), acc);
        }
        hsum_float_8(acc) + summs
    }
}

#[inline(always)]
pub(crate) fn vec_dot_q8k_q8k(n: usize, xs: &[BlockQ8K], ys: &[BlockQ8K]) -> f32 {
    debug_assert!(
        n.is_multiple_of(QK_K),
        "vec_dot_q8k_8k: {n} is not divisible by {QK_K}"
    );
    unsafe {
        let mut acc = _mm256_setzero_ps();
        for (xs, ys) in xs.iter().zip(ys.iter()) {
            let mut sumi = _mm256_setzero_si256();
            let x_qs = xs.qs.as_ptr();
            let y_qs = ys.qs.as_ptr();
            for j in (0..QK_K).step_by(32) {
                let xs = _mm256_loadu_si256(x_qs.add(j) as *const __m256i);
                let ys = _mm256_loadu_si256(y_qs.add(j) as *const __m256i);

                let xs0 = _mm256_cvtepi8_epi16(_mm256_extracti128_si256(xs, 0));
                let ys0 = _mm256_cvtepi8_epi16(_mm256_extracti128_si256(ys, 0));
                sumi = _mm256_add_epi32(sumi, _mm256_madd_epi16(xs0, ys0));

                let xs1 = _mm256_cvtepi8_epi16(_mm256_extracti128_si256(xs, 1));
                let ys1 = _mm256_cvtepi8_epi16(_mm256_extracti128_si256(ys, 1));
                sumi = _mm256_add_epi32(sumi, _mm256_madd_epi16(xs1, ys1));
            }
            let d = _mm256_set1_ps(xs.d * ys.d);
            acc = _mm256_fmadd_ps(d, _mm256_cvtepi32_ps(sumi), acc);
        }
        hsum_float_8(acc)
    }
}
