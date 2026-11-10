#![allow(dead_code)]
use libc::{c_char, c_double, c_float, c_int, c_long, c_ulong};

mod ffi {
    use super::*;
    extern "C" {
        // It would be nice to be able to switch to the NEWLAPACK version of the function but this
        // seems to trigger some link error. Available function names can be seen here:
        // /Library/Developer/CommandLineTools/SDKs/MacOSX13.3.sdk/System/Library/Frameworks/Accelerate.framework/Versions/A/Accelerate.tbd
        #[link_name = "sgemm_"]
        pub fn sgemm_ffi(
            transa: *const c_char,
            transb: *const c_char,
            m: *const c_int,
            n: *const c_int,
            k: *const c_int,
            alpha: *const c_float,
            a: *const c_float,
            lda: *const c_int,
            b: *const c_float,
            ldb: *const c_int,
            beta: *const c_float,
            c: *mut c_float,
            ldc: *const c_int,
        );
        #[link_name = "dgemm_"]
        pub fn dgemm_ffi(
            transa: *const c_char,
            transb: *const c_char,
            m: *const c_int,
            n: *const c_int,
            k: *const c_int,
            alpha: *const c_double,
            a: *const c_double,
            lda: *const c_int,
            b: *const c_double,
            ldb: *const c_int,
            beta: *const c_double,
            c: *mut c_double,
            ldc: *const c_int,
        );

        pub fn vvexpf(dst: *mut c_float, src: *const c_float, len: *const c_int);
        pub fn vvexp(dst: *mut c_double, src: *const c_double, len: *const c_int);
        pub fn vvsqrtf(dst: *mut c_float, src: *const c_float, len: *const c_int);
        pub fn vvsqrt(dst: *mut c_double, src: *const c_double, len: *const c_int);
        pub fn vvsinf(dst: *mut c_float, src: *const c_float, len: *const c_int);
        pub fn vvsin(dst: *mut c_double, src: *const c_double, len: *const c_int);
        pub fn vvcosf(dst: *mut c_float, src: *const c_float, len: *const c_int);
        pub fn vvcos(dst: *mut c_double, src: *const c_double, len: *const c_int);
        pub fn vvlogf(dst: *mut c_float, src: *const c_float, len: *const c_int);
        pub fn vvlog(dst: *mut c_double, src: *const c_double, len: *const c_int);
        pub fn vvtanhf(dst: *mut c_float, src: *const c_float, len: *const c_int);
        pub fn vvtanh(dst: *mut c_double, src: *const c_double, len: *const c_int);

        pub fn vDSP_vaddD(
            _: *const c_double,
            _: c_long,
            _: *const c_double,
            _: c_long,
            _: *mut c_double,
            _: c_long,
            _: c_ulong,
        );
        pub fn vDSP_vadd(
            _: *const c_float,
            _: c_long,
            _: *const c_float,
            _: c_long,
            _: *mut c_float,
            _: c_long,
            _: c_ulong,
        );
        pub fn vDSP_vsubD(
            _: *const c_double,
            _: c_long,
            _: *const c_double,
            _: c_long,
            _: *mut c_double,
            _: c_long,
            _: c_ulong,
        );
        pub fn vDSP_vsub(
            _: *const c_float,
            _: c_long,
            _: *const c_float,
            _: c_long,
            _: *mut c_float,
            _: c_long,
            _: c_ulong,
        );
        pub fn vDSP_vmulD(
            _: *const c_double,
            _: c_long,
            _: *const c_double,
            _: c_long,
            _: *mut c_double,
            _: c_long,
            _: c_ulong,
        );
        pub fn vDSP_vmul(
            _: *const c_float,
            _: c_long,
            _: *const c_float,
            _: c_long,
            _: *mut c_float,
            _: c_long,
            _: c_ulong,
        );
        pub fn vDSP_vdivD(
            _: *const c_double,
            _: c_long,
            _: *const c_double,
            _: c_long,
            _: *mut c_double,
            _: c_long,
            _: c_ulong,
        );
        pub fn vDSP_vdiv(
            _: *const c_float,
            _: c_long,
            _: *const c_float,
            _: c_long,
            _: *mut c_float,
            _: c_long,
            _: c_ulong,
        );
        pub fn vDSP_vminD(
            _: *const c_double,
            _: c_long,
            _: *const c_double,
            _: c_long,
            _: *mut c_double,
            _: c_long,
            _: c_ulong,
        );
        pub fn vDSP_vmin(
            _: *const c_float,
            _: c_long,
            _: *const c_float,
            _: c_long,
            _: *mut c_float,
            _: c_long,
            _: c_ulong,
        );
        pub fn vDSP_vmaxD(
            _: *const c_double,
            _: c_long,
            _: *const c_double,
            _: c_long,
            _: *mut c_double,
            _: c_long,
            _: c_ulong,
        );
        pub fn vDSP_vmax(
            _: *const c_float,
            _: c_long,
            _: *const c_float,
            _: c_long,
            _: *mut c_float,
            _: c_long,
            _: c_ulong,
        );
    }
}

#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn sgemm(
    transa: u8,
    transb: u8,
    m: i32,
    n: i32,
    k: i32,
    alpha: f32,
    a: &[f32],
    lda: i32,
    b: &[f32],
    ldb: i32,
    beta: f32,
    c: &mut [f32],
    ldc: i32,
) {
    ffi::sgemm_ffi(
        &(transa as c_char),
        &(transb as c_char),
        &m,
        &n,
        &k,
        &alpha,
        a.as_ptr(),
        &lda,
        b.as_ptr(),
        &ldb,
        &beta,
        c.as_mut_ptr(),
        &ldc,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline]
pub unsafe fn dgemm(
    transa: u8,
    transb: u8,
    m: i32,
    n: i32,
    k: i32,
    alpha: f64,
    a: &[f64],
    lda: i32,
    b: &[f64],
    ldb: i32,
    beta: f64,
    c: &mut [f64],
    ldc: i32,
) {
    ffi::dgemm_ffi(
        &(transa as c_char),
        &(transb as c_char),
        &m,
        &n,
        &k,
        &alpha,
        a.as_ptr(),
        &lda,
        b.as_ptr(),
        &ldb,
        &beta,
        c.as_mut_ptr(),
        &ldc,
    )
}

#[inline]
pub fn vs_exp(a: &[f32], y: &mut [f32]) {
    let a_len = a.len();
    let y_len = y.len();
    if a_len != y_len {
        panic!("a and y have different lengths {a_len} <> {y_len}")
    }
    unsafe { ffi::vvexpf(y.as_mut_ptr(), a.as_ptr(), &(a_len as i32)) }
}

#[inline]
pub fn vd_exp(a: &[f64], y: &mut [f64]) {
    let a_len = a.len();
    let y_len = y.len();
    if a_len != y_len {
        panic!("a and y have different lengths {a_len} <> {y_len}")
    }
    unsafe { ffi::vvexp(y.as_mut_ptr(), a.as_ptr(), &(a_len as i32)) }
}

#[inline]
pub fn vs_sqrt(a: &[f32], y: &mut [f32]) {
    let a_len = a.len();
    let y_len = y.len();
    if a_len != y_len {
        panic!("a and y have different lengths {a_len} <> {y_len}")
    }
    unsafe { ffi::vvsqrtf(y.as_mut_ptr(), a.as_ptr(), &(a_len as i32)) }
}

#[inline]
pub fn vd_sqrt(a: &[f64], y: &mut [f64]) {
    let a_len = a.len();
    let y_len = y.len();
    if a_len != y_len {
        panic!("a and y have different lengths {a_len} <> {y_len}")
    }
    unsafe { ffi::vvsqrt(y.as_mut_ptr(), a.as_ptr(), &(a_len as i32)) }
}

#[inline]
pub fn vs_sin(a: &[f32], y: &mut [f32]) {
    let a_len = a.len();
    let y_len = y.len();
    if a_len != y_len {
        panic!("a and y have different lengths {a_len} <> {y_len}")
    }
    unsafe { ffi::vvsinf(y.as_mut_ptr(), a.as_ptr(), &(a_len as i32)) }
}

#[inline]
pub fn vd_sin(a: &[f64], y: &mut [f64]) {
    let a_len = a.len();
    let y_len = y.len();
    if a_len != y_len {
        panic!("a and y have different lengths {a_len} <> {y_len}")
    }
    unsafe { ffi::vvsin(y.as_mut_ptr(), a.as_ptr(), &(a_len as i32)) }
}
#[inline]
pub fn vs_cos(a: &[f32], y: &mut [f32]) {
    let a_len = a.len();
    let y_len = y.len();
    if a_len != y_len {
        panic!("a and y have different lengths {a_len} <> {y_len}")
    }
    unsafe { ffi::vvcosf(y.as_mut_ptr(), a.as_ptr(), &(a_len as i32)) }
}

#[inline]
pub fn vd_cos(a: &[f64], y: &mut [f64]) {
    let a_len = a.len();
    let y_len = y.len();
    if a_len != y_len {
        panic!("a and y have different lengths {a_len} <> {y_len}")
    }
    unsafe { ffi::vvcos(y.as_mut_ptr(), a.as_ptr(), &(a_len as i32)) }
}
#[inline]
pub fn vs_tanh(a: &[f32], y: &mut [f32]) {
    let a_len = a.len();
    let y_len = y.len();
    if a_len != y_len {
        panic!("a and y have different lengths {a_len} <> {y_len}")
    }
    unsafe { ffi::vvtanhf(y.as_mut_ptr(), a.as_ptr(), &(a_len as i32)) }
}

#[inline]
pub fn vd_tanh(a: &[f64], y: &mut [f64]) {
    let a_len = a.len();
    let y_len = y.len();
    if a_len != y_len {
        panic!("a and y have different lengths {a_len} <> {y_len}")
    }
    unsafe { ffi::vvtanh(y.as_mut_ptr(), a.as_ptr(), &(a_len as i32)) }
}

#[inline]
pub fn vs_ln(a: &[f32], y: &mut [f32]) {
    let a_len = a.len();
    let y_len = y.len();
    if a_len != y_len {
        panic!("a and y have different lengths {a_len} <> {y_len}")
    }
    unsafe { ffi::vvlogf(y.as_mut_ptr(), a.as_ptr(), &(a_len as i32)) }
}

#[inline]
pub fn vd_ln(a: &[f64], y: &mut [f64]) {
    let a_len = a.len();
    let y_len = y.len();
    if a_len != y_len {
        panic!("a and y have different lengths {a_len} <> {y_len}")
    }
    unsafe { ffi::vvlog(y.as_mut_ptr(), a.as_ptr(), &(a_len as i32)) }
}

#[inline]
pub fn vs_sqr(a: &[f32], y: &mut [f32]) {
    let a_len = a.len();
    let y_len = y.len();
    if a_len != y_len {
        panic!("a and y have different lengths {a_len} <> {y_len}")
    }
    y.iter_mut().zip(a.iter()).for_each(|(y, a)| *y = *a * *a)
}

#[inline]
pub fn vd_sqr(a: &[f64], y: &mut [f64]) {
    let a_len = a.len();
    let y_len = y.len();
    if a_len != y_len {
        panic!("a and y have different lengths {a_len} <> {y_len}")
    }
    y.iter_mut().zip(a.iter()).for_each(|(y, a)| *y = *a * *a)
}

#[inline]
pub fn vs_tanh_inplace(y: &mut [f32]) {
    unsafe { ffi::vvtanhf(y.as_mut_ptr(), y.as_ptr(), &(y.len() as i32)) }
}

#[inline]
pub fn vd_tanh_inplace(y: &mut [f64]) {
    unsafe { ffi::vvtanh(y.as_mut_ptr(), y.as_ptr(), &(y.len() as i32)) }
}

#[inline]
pub fn vs_exp_inplace(y: &mut [f32]) {
    unsafe { ffi::vvexpf(y.as_mut_ptr(), y.as_ptr(), &(y.len() as i32)) }
}

#[inline]
pub fn vd_exp_inplace(y: &mut [f64]) {
    unsafe { ffi::vvexp(y.as_mut_ptr(), y.as_ptr(), &(y.len() as i32)) }
}

#[inline]
pub fn vs_gelu(vs: &[f32], ys: &mut [f32]) {
    for (&v, y) in vs.iter().zip(ys.iter_mut()) {
        *y = (2.0f32 / std::f32::consts::PI).sqrt() * v * (1.0 + 0.044715 * v * v)
    }
    vs_tanh_inplace(ys);
    for (&v, y) in vs.iter().zip(ys.iter_mut()) {
        *y = 0.5 * v * (1.0 + *y)
    }
}

#[inline]
pub fn vd_gelu(vs: &[f64], ys: &mut [f64]) {
    for (&v, y) in vs.iter().zip(ys.iter_mut()) {
        *y = (2.0f64 / std::f64::consts::PI).sqrt() * v * (1.0 + 0.044715 * v * v)
    }
    vd_tanh_inplace(ys);
    for (&v, y) in vs.iter().zip(ys.iter_mut()) {
        *y = 0.5 * v * (1.0 + *y)
    }
}

#[inline]
pub fn vs_silu(vs: &[f32], ys: &mut [f32]) {
    for (&v, y) in vs.iter().zip(ys.iter_mut()) {
        *y = -v
    }
    vs_exp_inplace(ys);
    for (&v, y) in vs.iter().zip(ys.iter_mut()) {
        *y = v / (1.0 + *y)
    }
}

#[inline]
pub fn vd_silu(vs: &[f64], ys: &mut [f64]) {
    for (&v, y) in vs.iter().zip(ys.iter_mut()) {
        *y = -v
    }
    vd_exp_inplace(ys);
    for (&v, y) in vs.iter().zip(ys.iter_mut()) {
        *y = v / (1.0 + *y)
    }
}

macro_rules! binary_op {
    ($fn_name:ident, $ty:ty, $accelerate_name:ident) => {
        #[inline]
        pub fn $fn_name(a: &[$ty], b: &[$ty], y: &mut [$ty]) {
            let a_len = a.len();
            let b_len = b.len();
            let y_len = y.len();
            if a_len != y_len || b_len != y_len {
                panic!(
                    "{} a,b,y len mismatch {a_len} {b_len} {y_len}",
                    stringify!($fn_name)
                );
            }
            unsafe {
                // Weird quirk of accelerate, the rhs comes before the lhs.
                ffi::$accelerate_name(
                    b.as_ptr(),
                    1,
                    a.as_ptr(),
                    1,
                    y.as_mut_ptr(),
                    1,
                    a_len as u64,
                )
            }
        }
    };
}
binary_op!(vs_add, f32, vDSP_vadd);
binary_op!(vd_add, f64, vDSP_vaddD);
binary_op!(vs_sub, f32, vDSP_vsub);
binary_op!(vd_sub, f64, vDSP_vsubD);
binary_op!(vs_mul, f32, vDSP_vmul);
binary_op!(vd_mul, f64, vDSP_vmulD);
binary_op!(vs_div, f32, vDSP_vdiv);
binary_op!(vd_div, f64, vDSP_vdivD);
binary_op!(vs_max, f32, vDSP_vmax);
binary_op!(vd_max, f64, vDSP_vmaxD);
binary_op!(vs_min, f32, vDSP_vmin);
binary_op!(vd_min, f64, vDSP_vminD);
