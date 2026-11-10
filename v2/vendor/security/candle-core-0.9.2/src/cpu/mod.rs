//! Traits and methods for CPU-backed Tensors

pub mod erf;
pub mod kernels;

#[allow(unused)]
trait Cpu<const ARR: usize> {
    type Unit;
    type Array;
    const STEP: usize;
    const EPR: usize;

    fn n() -> usize;
    unsafe fn zero() -> Self::Unit;
    unsafe fn zero_array() -> Self::Array;
    unsafe fn load(mem_addr: *const f32) -> Self::Unit;
    unsafe fn vec_add(a: Self::Unit, b: Self::Unit) -> Self::Unit;
    unsafe fn vec_fma(a: Self::Unit, b: Self::Unit, c: Self::Unit) -> Self::Unit;
    unsafe fn vec_reduce(x: Self::Array, y: *mut f32);
    unsafe fn from_f32(v: f32) -> Self::Unit;
    unsafe fn vec_store(mem_addr: *mut f32, a: Self::Unit);
}

#[allow(unused)]
trait CpuF16<const ARR: usize> {
    type Unit;
    type Array;
    const STEP: usize;
    const EPR: usize;

    fn n() -> usize;
    unsafe fn zero() -> Self::Unit;
    unsafe fn zero_array() -> Self::Array;
    unsafe fn load(mem_addr: *const f16) -> Self::Unit;
    unsafe fn vec_add(a: Self::Unit, b: Self::Unit) -> Self::Unit;
    unsafe fn vec_fma(a: Self::Unit, b: Self::Unit, c: Self::Unit) -> Self::Unit;
    unsafe fn vec_reduce(x: Self::Array, y: *mut f32);
    unsafe fn from_f32(v: f32) -> Self::Unit;
    unsafe fn vec_store(mem_addr: *mut f16, a: Self::Unit);
}

#[allow(unused)]
trait CpuBF16<const ARR: usize> {
    type Unit;
    type Array;
    const STEP: usize;
    const EPR: usize;

    fn n() -> usize;
    unsafe fn zero() -> Self::Unit;
    unsafe fn zero_array() -> Self::Array;
    unsafe fn load(mem_addr: *const bf16) -> Self::Unit;
    unsafe fn vec_add(a: Self::Unit, b: Self::Unit) -> Self::Unit;
    unsafe fn vec_fma(a: Self::Unit, b: Self::Unit, c: Self::Unit) -> Self::Unit;
    unsafe fn vec_reduce(x: Self::Array, y: *mut f32);
    unsafe fn from_f32(v: f32) -> Self::Unit;
    unsafe fn vec_store(mem_addr: *mut bf16, a: Self::Unit);
}

use half::{bf16, f16};

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(target_feature = "avx2")]
pub mod avx;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(target_feature = "avx2")]
pub use avx::{CurrentCpu, CurrentCpuBF16, CurrentCpuF16};

#[cfg(target_arch = "wasm32")]
#[cfg(target_feature = "simd128")]
pub mod simd128;
#[cfg(target_arch = "wasm32")]
#[cfg(target_feature = "simd128")]
pub use simd128::CurrentCpu;

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
#[cfg(target_feature = "neon")]
pub mod neon;
#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
#[cfg(target_feature = "neon")]
pub use neon::CurrentCpu;

#[cfg(any(
    target_feature = "neon",
    target_feature = "avx2",
    target_feature = "simd128"
))]
#[inline(always)]
pub(crate) unsafe fn vec_dot_f32(a_row: *const f32, b_row: *const f32, c: *mut f32, k: usize) {
    let np = k & !(CurrentCpu::STEP - 1);

    let mut sum = CurrentCpu::zero_array();
    let mut ax = CurrentCpu::zero_array();
    let mut ay = CurrentCpu::zero_array();

    for i in (0..np).step_by(CurrentCpu::STEP) {
        for j in 0..CurrentCpu::n() {
            ax[j] = CurrentCpu::load(a_row.add(i + j * CurrentCpu::EPR));
            ay[j] = CurrentCpu::load(b_row.add(i + j * CurrentCpu::EPR));

            sum[j] = CurrentCpu::vec_fma(sum[j], ax[j], ay[j]);
        }
    }

    CurrentCpu::vec_reduce(sum, c);

    // leftovers
    for i in np..k {
        *c += *a_row.add(i) * (*b_row.add(i));
    }
}

#[cfg(not(any(
    target_feature = "neon",
    target_feature = "avx2",
    target_feature = "simd128"
)))]
#[inline(always)]
pub(crate) unsafe fn vec_dot_f32(a_row: *const f32, b_row: *const f32, c: *mut f32, k: usize) {
    // leftovers
    for i in 0..k {
        *c += *a_row.add(i) * (*b_row.add(i));
    }
}

#[cfg(any(
    target_feature = "neon",
    target_feature = "avx2",
    target_feature = "simd128"
))]
#[inline(always)]
pub(crate) unsafe fn vec_sum(row: *const f32, b: *mut f32, k: usize) {
    let np = k & !(CurrentCpu::STEP - 1);

    let mut sum = CurrentCpu::zero_array();
    let mut x = CurrentCpu::zero_array();

    for i in (0..np).step_by(CurrentCpu::STEP) {
        for j in 0..CurrentCpu::n() {
            x[j] = CurrentCpu::load(row.add(i + j * CurrentCpu::EPR));
            sum[j] = CurrentCpu::vec_add(sum[j], x[j]);
        }
    }

    CurrentCpu::vec_reduce(sum, b);

    // leftovers
    for i in np..k {
        *b += *row.add(i)
    }
}

#[cfg(not(any(
    target_feature = "neon",
    target_feature = "avx2",
    target_feature = "simd128"
)))]
#[inline(always)]
pub(crate) unsafe fn vec_sum(row: *const f32, b: *mut f32, k: usize) {
    *b = 0f32;
    for i in 0..k {
        *b += *row.add(i)
    }
}

#[cfg(target_feature = "avx2")]
#[inline(always)]
pub(crate) unsafe fn vec_dot_f16(a_row: *const f16, b_row: *const f16, c: *mut f32, k: usize) {
    let mut sumf = 0.0f32;
    let np = k & !(CurrentCpuF16::STEP - 1);

    let mut sum = CurrentCpuF16::zero_array();
    let mut ax = CurrentCpuF16::zero_array();
    let mut ay = CurrentCpuF16::zero_array();

    for i in (0..np).step_by(CurrentCpuF16::STEP) {
        for j in 0..CurrentCpuF16::n() {
            ax[j] = CurrentCpuF16::load(a_row.add(i + j * CurrentCpuF16::EPR));
            ay[j] = CurrentCpuF16::load(b_row.add(i + j * CurrentCpuF16::EPR));

            sum[j] = CurrentCpuF16::vec_fma(sum[j], ax[j], ay[j]);
        }
    }

    CurrentCpuF16::vec_reduce(sum, &mut sumf);

    // leftovers
    for i in np..k {
        sumf += (*a_row.add(i)).to_f32() * (*b_row.add(i)).to_f32();
    }
    *c = sumf;
}

#[cfg(target_feature = "avx2")]
#[inline(always)]
pub(crate) unsafe fn vec_dot_bf16(a_row: *const bf16, b_row: *const bf16, c: *mut f32, k: usize) {
    let mut sumf = 0.0f32;
    let np = k & !(CurrentCpuBF16::STEP - 1);

    let mut sum = CurrentCpuBF16::zero_array();
    let mut ax = CurrentCpuBF16::zero_array();
    let mut ay = CurrentCpuBF16::zero_array();

    for i in (0..np).step_by(CurrentCpuBF16::STEP) {
        for j in 0..CurrentCpuBF16::n() {
            ax[j] = CurrentCpuBF16::load(a_row.add(i + j * CurrentCpuBF16::EPR));
            ay[j] = CurrentCpuBF16::load(b_row.add(i + j * CurrentCpuBF16::EPR));

            sum[j] = CurrentCpuBF16::vec_fma(sum[j], ax[j], ay[j]);
        }
    }

    CurrentCpuBF16::vec_reduce(sum, &mut sumf);

    // leftovers
    for i in np..k {
        sumf += (*a_row.add(i)).to_f32() * (*b_row.add(i)).to_f32();
    }
    *c = sumf;
}

#[cfg(not(target_feature = "avx2"))]
#[inline(always)]
pub(crate) unsafe fn vec_dot_f16(a_row: *const f16, b_row: *const f16, c: *mut f32, k: usize) {
    // leftovers
    let mut sum = 0.0;
    for i in 0..k {
        sum += (*a_row.add(i)).to_f32() * (*b_row.add(i)).to_f32();
    }
    *c = sum;
}

#[cfg(not(target_feature = "avx2"))]
#[inline(always)]
pub(crate) unsafe fn vec_dot_bf16(a_row: *const bf16, b_row: *const bf16, c: *mut f32, k: usize) {
    // leftovers
    let mut sum = 0.0;
    for i in 0..k {
        sum += (*a_row.add(i)).to_f32() * (*b_row.add(i)).to_f32();
    }
    *c = sum;
}
