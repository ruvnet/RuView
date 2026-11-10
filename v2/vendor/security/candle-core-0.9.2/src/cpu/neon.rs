use super::Cpu;
#[cfg(target_arch = "arm")]
use core::arch::arm::*;

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

pub struct CurrentCpu {}

const STEP: usize = 16;
const EPR: usize = 4;
const ARR: usize = STEP / EPR;

impl CurrentCpu {
    #[cfg(target_arch = "aarch64")]
    unsafe fn reduce_one(x: float32x4_t) -> f32 {
        vaddvq_f32(x)
    }

    #[cfg(target_arch = "arm")]
    unsafe fn reduce_one(x: float32x4_t) -> f32 {
        vgetq_lane_f32(x, 0) + vgetq_lane_f32(x, 1) + vgetq_lane_f32(x, 2) + vgetq_lane_f32(x, 3)
    }
}

impl Cpu<ARR> for CurrentCpu {
    type Unit = float32x4_t;
    type Array = [float32x4_t; ARR];

    const STEP: usize = STEP;
    const EPR: usize = EPR;

    fn n() -> usize {
        ARR
    }

    unsafe fn zero() -> Self::Unit {
        vdupq_n_f32(0.0)
    }

    unsafe fn from_f32(x: f32) -> Self::Unit {
        vdupq_n_f32(x)
    }

    unsafe fn zero_array() -> Self::Array {
        [Self::zero(); ARR]
    }

    unsafe fn load(mem_addr: *const f32) -> Self::Unit {
        vld1q_f32(mem_addr)
    }

    unsafe fn vec_add(a: Self::Unit, b: Self::Unit) -> Self::Unit {
        vaddq_f32(a, b)
    }

    unsafe fn vec_fma(a: Self::Unit, b: Self::Unit, c: Self::Unit) -> Self::Unit {
        vfmaq_f32(a, b, c)
    }

    unsafe fn vec_store(mem_addr: *mut f32, a: Self::Unit) {
        vst1q_f32(mem_addr, a);
    }

    unsafe fn vec_reduce(mut x: Self::Array, y: *mut f32) {
        for i in 0..ARR / 2 {
            x[2 * i] = vaddq_f32(x[2 * i], x[2 * i + 1]);
        }
        for i in 0..ARR / 4 {
            x[4 * i] = vaddq_f32(x[4 * i], x[4 * i + 2]);
        }
        *y = Self::reduce_one(x[0]);
    }
}
