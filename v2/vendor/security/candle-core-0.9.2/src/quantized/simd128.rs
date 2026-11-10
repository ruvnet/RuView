use super::k_quants::{BlockQ2K, BlockQ4K, BlockQ4_0, BlockQ6K, BlockQ8K, BlockQ8_0, QK8_0, QK_K};
use byteorder::{ByteOrder, LittleEndian};
use half::f16;

use core::arch::wasm32::*;

#[inline(always)]
pub(crate) fn vec_dot_q4_0_q8_0(n: usize, xs: &[BlockQ4_0], ys: &[BlockQ8_0]) -> f32 {
    debug_assert!(
        n.is_multiple_of(QK8_0),
        "vec_dot_q4_0_q8_0: {n} is not divisible by {QK8_0}"
    );
    unsafe {
        let mut acc = f32x4_splat(0.0f32);
        for (x, y) in xs.iter().zip(ys.iter()) {
            let x1234 = v128_load(x.qs.as_ptr() as *const v128);
            let x12 = v128_and(x1234, u8x16_splat(0x0F));
            let x12 = i8x16_sub(x12, i8x16_splat(8));
            let x34 = u8x16_shr(x1234, 4);
            let x34 = i8x16_sub(x34, i8x16_splat(8));

            let x1 = i16x8_extend_low_i8x16(x12);
            let y1 = i16x8_load_extend_i8x8(y.qs.as_ptr());
            let sum_xy = i32x4_dot_i16x8(x1, y1);

            let x2 = i16x8_extend_high_i8x16(x12);
            let y2 = i16x8_load_extend_i8x8(y.qs.as_ptr().add(8));
            let sum_xy = i32x4_add(sum_xy, i32x4_dot_i16x8(x2, y2));

            let x3 = i16x8_extend_low_i8x16(x34);
            let y3 = i16x8_load_extend_i8x8(y.qs.as_ptr().add(16));
            let sum_xy = i32x4_add(sum_xy, i32x4_dot_i16x8(x3, y3));

            let x4 = i16x8_extend_high_i8x16(x34);
            let y4 = i16x8_load_extend_i8x8(y.qs.as_ptr().add(24));
            let sum_xy = i32x4_add(sum_xy, i32x4_dot_i16x8(x4, y4));

            let sum_xy = f32x4_convert_i32x4(sum_xy);

            // f32x4_relaxed_madd is nightly only.
            let d = f32x4_splat(f16::to_f32(x.d) * f16::to_f32(y.d));
            let scaled = f32x4_mul(sum_xy, d);
            acc = f32x4_add(acc, scaled)
        }
        let res = f32x4_extract_lane::<0>(acc)
            + f32x4_extract_lane::<1>(acc)
            + f32x4_extract_lane::<2>(acc)
            + f32x4_extract_lane::<3>(acc);
        res
    }
}

#[inline(always)]
pub(crate) fn vec_dot_q8_0_q8_0(n: usize, xs: &[BlockQ8_0], ys: &[BlockQ8_0]) -> f32 {
    debug_assert!(
        n.is_multiple_of(QK8_0),
        "vec_dot_q8_0_q8_0: {n} is not divisible by {QK8_0}"
    );
    unsafe {
        let mut acc = f32x4_splat(0.0f32);
        for (x, y) in xs.iter().zip(ys.iter()) {
            let x1 = i16x8_load_extend_i8x8(x.qs.as_ptr());
            let y1 = i16x8_load_extend_i8x8(y.qs.as_ptr());
            let sum_xy = i32x4_dot_i16x8(x1, y1);

            let x2 = i16x8_load_extend_i8x8(x.qs.as_ptr().add(8));
            let y2 = i16x8_load_extend_i8x8(y.qs.as_ptr().add(8));
            let sum_xy = i32x4_add(sum_xy, i32x4_dot_i16x8(x2, y2));

            let x3 = i16x8_load_extend_i8x8(x.qs.as_ptr().add(16));
            let y3 = i16x8_load_extend_i8x8(y.qs.as_ptr().add(16));
            let sum_xy = i32x4_add(sum_xy, i32x4_dot_i16x8(x3, y3));

            let x4 = i16x8_load_extend_i8x8(x.qs.as_ptr().add(24));
            let y4 = i16x8_load_extend_i8x8(y.qs.as_ptr().add(24));
            let sum_xy = i32x4_add(sum_xy, i32x4_dot_i16x8(x4, y4));

            let sum_xy = f32x4_convert_i32x4(sum_xy);

            // f32x4_relaxed_madd is nightly only.
            let d = f32x4_splat(f16::to_f32(x.d) * f16::to_f32(y.d));
            let scaled = f32x4_mul(sum_xy, d);
            acc = f32x4_add(acc, scaled)
        }
        let res = f32x4_extract_lane::<0>(acc)
            + f32x4_extract_lane::<1>(acc)
            + f32x4_extract_lane::<2>(acc)
            + f32x4_extract_lane::<3>(acc);
        res
    }
}

#[inline(always)]
pub(crate) fn vec_dot_q2k_q8k(n: usize, xs: &[BlockQ2K], ys: &[BlockQ8K]) -> f32 {
    debug_assert!(
        n.is_multiple_of(QK_K),
        "vec_dot_q2k_q8k: {n} is not divisible by {QK_K}"
    );
    unsafe {
        let mut sumf = f32x4_splat(0f32);
        for (x, y) in xs.iter().zip(ys.iter()) {
            let mut q2: &[_] = &x.qs;
            let mut q8: &[_] = &y.qs;
            let sc = &x.scales;

            let mut summs = i32x4_splat(0);
            for i in (0..(QK_K / 16)).step_by(4) {
                let bsums = i32x4_load_extend_i16x4(y.bsums.as_ptr().add(i));
                let scales = i32x4_shr(
                    i32x4(
                        sc[i] as i32,
                        sc[i + 1] as i32,
                        sc[i + 2] as i32,
                        sc[i + 3] as i32,
                    ),
                    4,
                );
                summs = i32x4_add(summs, i32x4_mul(bsums, scales))
            }
            let summs = f32x4_convert_i32x4(summs);

            let dall = y.d * x.d.to_f32();
            let dmin = y.d * x.dmin.to_f32();

            let mut isum = i32x4_splat(0);
            let mut is = 0;
            for _ in 0..(QK_K / 128) {
                let mut shift = 0;
                for _ in 0..4 {
                    let d = (sc[is] & 0xF) as i32;
                    is += 1;
                    let mut isuml = i16x8_splat(0);
                    for l in (0..16).step_by(8) {
                        let q8 = i16x8_load_extend_i8x8(q8.as_ptr().add(l));
                        let q2 = i16x8_load_extend_u8x8(q2.as_ptr().add(l));
                        let q2 = v128_and(i16x8_shr(q2, shift), i16x8_splat(3));
                        isuml = i16x8_add(isuml, i16x8_mul(q2, q8))
                    }
                    let dd = i32x4_splat(d);
                    isum = i32x4_add(isum, i32x4_mul(i32x4_extend_low_i16x8(isuml), dd));
                    isum = i32x4_add(isum, i32x4_mul(i32x4_extend_high_i16x8(isuml), dd));
                    let d = (sc[is] & 0xF) as i32;
                    is += 1;
                    let mut isuml = i16x8_splat(0);
                    for l in (16..32).step_by(8) {
                        let q8 = i16x8_load_extend_i8x8(q8.as_ptr().add(l));
                        let q2 = i16x8_load_extend_u8x8(q2.as_ptr().add(l));
                        let q2 = v128_and(i16x8_shr(q2, shift), i16x8_splat(3));
                        isuml = i16x8_add(isuml, i16x8_mul(q2, q8))
                    }
                    let dd = i32x4_splat(d);
                    isum = i32x4_add(isum, i32x4_mul(i32x4_extend_low_i16x8(isuml), dd));
                    isum = i32x4_add(isum, i32x4_mul(i32x4_extend_high_i16x8(isuml), dd));
                    shift += 2;
                    // adjust the indexing
                    q8 = &q8[32..];
                }
                // adjust the indexing
                q2 = &q2[32..];
            }
            let isum = f32x4_convert_i32x4(isum);
            sumf = f32x4_add(
                sumf,
                f32x4_sub(
                    f32x4_mul(isum, f32x4_splat(dall)),
                    f32x4_mul(summs, f32x4_splat(dmin)),
                ),
            );
        }
        let sumf = f32x4_extract_lane::<0>(sumf)
            + f32x4_extract_lane::<1>(sumf)
            + f32x4_extract_lane::<2>(sumf)
            + f32x4_extract_lane::<3>(sumf);
        sumf
    }
}

#[inline(always)]
pub(crate) fn vec_dot_q4k_q8k(n: usize, xs: &[BlockQ4K], ys: &[BlockQ8K]) -> f32 {
    debug_assert!(
        n.is_multiple_of(QK_K),
        "vec_dot_q4k_q8k: {n} is not divisible by {QK_K}"
    );
    const KMASK1: u32 = 0x3f3f3f3f;
    const KMASK2: u32 = 0x0f0f0f0f;
    const KMASK3: u32 = 0x03030303;

    let mut utmp: [u32; 4] = [0; 4];
    let mut scales: [u8; 8] = [0; 8];
    let mut mins: [u8; 8] = [0; 8];

    let mut aux8: [u8; QK_K] = [0; QK_K];
    let mut sums = f32x4_splat(0f32);
    unsafe {
        for (y, x) in ys.iter().zip(xs.iter()) {
            let q4 = &x.qs;
            let q8 = &y.qs;

            for j in 0..QK_K / 64 {
                let q4_1 = v128_load(q4.as_ptr().add(32 * j) as *const v128);
                let q4_2 = v128_load(q4.as_ptr().add(32 * j + 16) as *const v128);
                v128_store(
                    aux8.as_mut_ptr().add(64 * j) as *mut v128,
                    v128_and(q4_1, u8x16_splat(0x0F)),
                );
                v128_store(
                    aux8.as_mut_ptr().add(64 * j + 16) as *mut v128,
                    v128_and(q4_2, u8x16_splat(0x0F)),
                );
                v128_store(
                    aux8.as_mut_ptr().add(64 * j + 32) as *mut v128,
                    u8x16_shr(q4_1, 4),
                );
                v128_store(
                    aux8.as_mut_ptr().add(64 * j + 48) as *mut v128,
                    u8x16_shr(q4_2, 4),
                );
            }

            LittleEndian::read_u32_into(&x.scales, &mut utmp[0..3]);

            utmp[3] = ((utmp[2] >> 4) & KMASK2) | (((utmp[1] >> 6) & KMASK3) << 4);
            let uaux = utmp[1] & KMASK1;
            utmp[1] = (utmp[2] & KMASK2) | (((utmp[0] >> 6) & KMASK3) << 4);
            utmp[2] = uaux;
            utmp[0] &= KMASK1;

            //extract scales and mins
            LittleEndian::write_u32_into(&utmp[0..2], &mut scales);
            LittleEndian::write_u32_into(&utmp[2..4], &mut mins);

            let mut sumi = i32x4_splat(0);
            for j in (0..QK_K / 16).step_by(4) {
                let bsums = i32x4_load_extend_i16x4(y.bsums.as_ptr().add(j));
                let (m1, m2) = (mins[j / 2] as i32, mins[j / 2 + 1] as i32);
                let mins = i32x4(m1, m1, m2, m2);
                sumi = i32x4_add(sumi, i32x4_mul(bsums, mins));
            }

            let mut aux32 = i32x4_splat(0i32);
            for (scale_i, scale) in scales.iter().enumerate() {
                let scale = i32x4_splat(*scale as i32);
                for j in 0..4 {
                    let i = 32 * scale_i + 8 * j;
                    let q8 = i16x8_load_extend_i8x8(q8.as_ptr().add(i));
                    let aux8 = i16x8_load_extend_u8x8(aux8.as_ptr().add(i));
                    let aux16 = i16x8_mul(q8, aux8);
                    aux32 = i32x4_add(aux32, i32x4_mul(scale, i32x4_extend_low_i16x8(aux16)));
                    aux32 = i32x4_add(aux32, i32x4_mul(scale, i32x4_extend_high_i16x8(aux16)));
                }
            }
            let aux32 = f32x4_convert_i32x4(aux32);
            let d = f32x4_splat(x.d.to_f32() * y.d);
            sums = f32x4_add(sums, f32x4_mul(aux32, d));
            let dmin = x.dmin.to_f32() * y.d;
            let dmin = f32x4_splat(dmin);
            let sumi = f32x4_convert_i32x4(sumi);
            sums = f32x4_sub(sums, f32x4_mul(sumi, dmin));
        }
        let sums = f32x4_extract_lane::<0>(sums)
            + f32x4_extract_lane::<1>(sums)
            + f32x4_extract_lane::<2>(sums)
            + f32x4_extract_lane::<3>(sums);
        sums
    }
}

#[inline(always)]
pub(crate) fn vec_dot_q6k_q8k(n: usize, xs: &[BlockQ6K], ys: &[BlockQ8K]) -> f32 {
    debug_assert!(
        n.is_multiple_of(QK_K),
        "vec_dot_q6k_q8k: {n} is not divisible by {QK_K}"
    );
    let mut aux8 = [0i8; QK_K];
    unsafe {
        let mut sums = f32x4_splat(0f32);

        for (x, y) in xs.iter().zip(ys.iter()) {
            let q4 = &x.ql;
            let qh = &x.qh;
            let q8 = &y.qs;
            let mut aux32 = f32x4_splat(0f32);

            for j in (0..QK_K).step_by(128) {
                let aux8 = aux8.as_mut_ptr().add(j);
                let q4 = &q4.as_ptr().add(j / 2);
                let qh = &qh.as_ptr().add(j / 4);
                for l in (0..32).step_by(16) {
                    // aux8[l] = (((q4[l] & 0xF) | ((qh[l] & 3) << 4)) as i32 - 32) as i8;
                    let a8 = v128_or(
                        v128_and(v128_load(q4.add(l) as *const v128), u8x16_splat(0xF)),
                        u8x16_shl(
                            v128_and(v128_load(qh.add(l) as *const v128), u8x16_splat(3)),
                            4,
                        ),
                    );
                    let a8_low = i16x8_sub(i16x8_extend_low_u8x16(a8), i16x8_splat(32));
                    let a8_high = i16x8_sub(i16x8_extend_high_u8x16(a8), i16x8_splat(32));
                    v128_store(
                        aux8.add(l) as *mut v128,
                        i8x16_narrow_i16x8(a8_low, a8_high),
                    );

                    // aux8[l + 32] =
                    //    (((q4[l + 32] & 0xF) | (((qh[l] >> 2) & 3) << 4)) as i32 - 32) as i8;
                    let a8 = v128_or(
                        v128_and(v128_load(q4.add(l + 32) as *const v128), u8x16_splat(0xF)),
                        u8x16_shl(
                            v128_and(
                                u8x16_shr(v128_load(qh.add(l) as *const v128), 2),
                                u8x16_splat(3),
                            ),
                            4,
                        ),
                    );
                    let a8_low = i16x8_sub(i16x8_extend_low_u8x16(a8), i16x8_splat(32));
                    let a8_high = i16x8_sub(i16x8_extend_high_u8x16(a8), i16x8_splat(32));
                    v128_store(
                        aux8.add(l + 32) as *mut v128,
                        i8x16_narrow_i16x8(a8_low, a8_high),
                    );

                    // aux8[l + 64] = (((q4[l] >> 4) | (((qh[l] >> 4) & 3) << 4)) as i32 - 32) as i8;
                    let a8 = v128_or(
                        u8x16_shr(v128_load(q4.add(l) as *const v128), 4),
                        u8x16_shl(
                            v128_and(
                                u8x16_shr(v128_load(qh.add(l) as *const v128), 4),
                                u8x16_splat(3),
                            ),
                            4,
                        ),
                    );
                    let a8_low = i16x8_sub(i16x8_extend_low_u8x16(a8), i16x8_splat(32));
                    let a8_high = i16x8_sub(i16x8_extend_high_u8x16(a8), i16x8_splat(32));
                    v128_store(
                        aux8.add(l + 64) as *mut v128,
                        i8x16_narrow_i16x8(a8_low, a8_high),
                    );

                    // aux8[l + 96] =
                    //    (((q4[l + 32] >> 4) | (((qh[l] >> 6) & 3) << 4)) as i32 - 32) as i8;
                    let a8 = v128_or(
                        u8x16_shr(v128_load(q4.add(l + 32) as *const v128), 4),
                        u8x16_shl(
                            v128_and(
                                u8x16_shr(v128_load(qh.add(l) as *const v128), 6),
                                u8x16_splat(3),
                            ),
                            4,
                        ),
                    );
                    let a8_low = i16x8_sub(i16x8_extend_low_u8x16(a8), i16x8_splat(32));
                    let a8_high = i16x8_sub(i16x8_extend_high_u8x16(a8), i16x8_splat(32));
                    v128_store(
                        aux8.add(l + 96) as *mut v128,
                        i8x16_narrow_i16x8(a8_low, a8_high),
                    );
                }
            }

            for (j, &scale) in x.scales.iter().enumerate() {
                let scale = f32x4_splat(scale as f32);
                for offset in [0, 8] {
                    let aux16 = i16x8_mul(
                        i16x8_load_extend_i8x8(q8.as_ptr().add(16 * j + offset)),
                        i16x8_load_extend_i8x8(aux8.as_ptr().add(16 * j + offset)),
                    );
                    aux32 = f32x4_add(
                        aux32,
                        f32x4_mul(f32x4_convert_i32x4(i32x4_extend_low_i16x8(aux16)), scale),
                    );
                    aux32 = f32x4_add(
                        aux32,
                        f32x4_mul(f32x4_convert_i32x4(i32x4_extend_high_i16x8(aux16)), scale),
                    );
                }
            }

            let d = f32x4_splat(x.d.to_f32() * y.d);
            sums = f32x4_add(sums, f32x4_mul(aux32, d));
        }
        let sums = f32x4_extract_lane::<0>(sums)
            + f32x4_extract_lane::<1>(sums)
            + f32x4_extract_lane::<2>(sums)
            + f32x4_extract_lane::<3>(sums);
        sums
    }
}

#[inline(always)]
pub(crate) fn vec_dot_q8k_q8k(n: usize, xs: &[BlockQ8K], ys: &[BlockQ8K]) -> f32 {
    debug_assert!(
        n.is_multiple_of(QK_K),
        "vec_dot_q8k_q8k: {n} is not divisible by {QK_K}"
    );
    unsafe {
        let mut acc = f32x4_splat(0.0f32);
        for (xs, ys) in xs.iter().zip(ys.iter()) {
            let x_qs = xs.qs.as_ptr();
            let y_qs = ys.qs.as_ptr();
            let mut sumi = i32x4_splat(0);
            for j in (0..QK_K).step_by(8) {
                let xs = i16x8_load_extend_i8x8(x_qs.add(j));
                let ys = i16x8_load_extend_i8x8(y_qs.add(j));
                let sum_xy = i32x4_dot_i16x8(xs, ys);
                sumi = i32x4_add(sumi, sum_xy)
            }
            let d = f32x4_splat(xs.d * ys.d);
            acc = f32x4_add(acc, f32x4_mul(f32x4_convert_i32x4(sumi), d))
        }
        let res = f32x4_extract_lane::<0>(acc)
            + f32x4_extract_lane::<1>(acc)
            + f32x4_extract_lane::<2>(acc)
            + f32x4_extract_lane::<3>(acc);
        res
    }
}
