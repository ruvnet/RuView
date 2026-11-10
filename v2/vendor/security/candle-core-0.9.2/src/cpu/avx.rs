use super::{Cpu, CpuBF16, CpuF16};
#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

use half::{bf16, f16};

pub struct CurrentCpu {}

const STEP: usize = 32;
const EPR: usize = 8;
const ARR: usize = STEP / EPR;

impl Cpu<ARR> for CurrentCpu {
    type Unit = __m256;
    type Array = [__m256; ARR];

    const STEP: usize = STEP;
    const EPR: usize = EPR;

    fn n() -> usize {
        ARR
    }

    unsafe fn zero() -> Self::Unit {
        _mm256_setzero_ps()
    }

    unsafe fn zero_array() -> Self::Array {
        [Self::zero(); ARR]
    }

    unsafe fn from_f32(v: f32) -> Self::Unit {
        _mm256_set1_ps(v)
    }

    unsafe fn load(mem_addr: *const f32) -> Self::Unit {
        _mm256_loadu_ps(mem_addr)
    }

    unsafe fn vec_add(a: Self::Unit, b: Self::Unit) -> Self::Unit {
        _mm256_add_ps(a, b)
    }

    unsafe fn vec_fma(a: Self::Unit, b: Self::Unit, c: Self::Unit) -> Self::Unit {
        _mm256_add_ps(_mm256_mul_ps(b, c), a)
    }

    unsafe fn vec_store(mem_addr: *mut f32, a: Self::Unit) {
        _mm256_storeu_ps(mem_addr, a);
    }

    unsafe fn vec_reduce(mut x: Self::Array, y: *mut f32) {
        for i in 0..ARR / 2 {
            x[2 * i] = _mm256_add_ps(x[2 * i], x[2 * i + 1]);
        }
        for i in 0..ARR / 4 {
            x[4 * i] = _mm256_add_ps(x[4 * i], x[4 * i + 2]);
        }
        #[allow(clippy::reversed_empty_ranges)]
        for i in 0..ARR / 8 {
            x[8 * i] = _mm256_add_ps(x[8 * i], x[8 * i + 4]);
        }
        let t0 = _mm_add_ps(_mm256_castps256_ps128(x[0]), _mm256_extractf128_ps(x[0], 1));
        let t1 = _mm_hadd_ps(t0, t0);
        *y = _mm_cvtss_f32(_mm_hadd_ps(t1, t1));
    }
}

pub struct CurrentCpuF16 {}
impl CpuF16<ARR> for CurrentCpuF16 {
    type Unit = __m256;
    type Array = [__m256; ARR];

    const STEP: usize = STEP;
    const EPR: usize = EPR;

    fn n() -> usize {
        ARR
    }

    unsafe fn zero() -> Self::Unit {
        _mm256_setzero_ps()
    }

    unsafe fn zero_array() -> Self::Array {
        [Self::zero(); ARR]
    }

    unsafe fn from_f32(v: f32) -> Self::Unit {
        _mm256_set1_ps(v)
    }

    #[cfg(target_feature = "f16c")]
    unsafe fn load(mem_addr: *const f16) -> Self::Unit {
        _mm256_cvtph_ps(_mm_loadu_si128(mem_addr as *const __m128i))
    }

    #[cfg(not(target_feature = "f16c"))]
    unsafe fn load(mem_addr: *const f16) -> Self::Unit {
        let mut tmp = [0.0f32; 8];
        for i in 0..8 {
            tmp[i] = (*mem_addr.add(i)).to_f32();
        }
        _mm256_loadu_ps(tmp.as_ptr())
    }

    unsafe fn vec_add(a: Self::Unit, b: Self::Unit) -> Self::Unit {
        _mm256_add_ps(a, b)
    }

    unsafe fn vec_fma(a: Self::Unit, b: Self::Unit, c: Self::Unit) -> Self::Unit {
        _mm256_add_ps(_mm256_mul_ps(b, c), a)
    }

    #[cfg(target_feature = "f16c")]
    unsafe fn vec_store(mem_addr: *mut f16, a: Self::Unit) {
        _mm_storeu_si128(mem_addr as *mut __m128i, _mm256_cvtps_ph(a, 0))
    }

    #[cfg(not(target_feature = "f16c"))]
    unsafe fn vec_store(mem_addr: *mut f16, a: Self::Unit) {
        let mut tmp = [0.0f32; 8];
        _mm256_storeu_ps(tmp.as_mut_ptr(), a);
        for i in 0..8 {
            *mem_addr.add(i) = f16::from_f32(tmp[i]);
        }
    }

    unsafe fn vec_reduce(mut x: Self::Array, y: *mut f32) {
        let mut offset = ARR >> 1;
        for i in 0..offset {
            x[i] = _mm256_add_ps(x[i], x[offset + i]);
        }
        offset >>= 1;
        for i in 0..offset {
            x[i] = _mm256_add_ps(x[i], x[offset + i]);
        }
        offset >>= 1;
        for i in 0..offset {
            x[i] = _mm256_add_ps(x[i], x[offset + i]);
        }
        let t0 = _mm_add_ps(_mm256_castps256_ps128(x[0]), _mm256_extractf128_ps(x[0], 1));
        let t1 = _mm_hadd_ps(t0, t0);
        *y = _mm_cvtss_f32(_mm_hadd_ps(t1, t1));
    }
}

pub struct CurrentCpuBF16 {}
impl CpuBF16<ARR> for CurrentCpuBF16 {
    type Unit = __m256;
    type Array = [__m256; ARR];

    const STEP: usize = STEP;
    const EPR: usize = EPR;

    fn n() -> usize {
        ARR
    }

    unsafe fn zero() -> Self::Unit {
        _mm256_setzero_ps()
    }

    unsafe fn zero_array() -> Self::Array {
        [Self::zero(); ARR]
    }

    unsafe fn from_f32(v: f32) -> Self::Unit {
        _mm256_set1_ps(v)
    }

    #[cfg(target_feature = "f16c")]
    unsafe fn load(mem_addr: *const bf16) -> Self::Unit {
        _mm256_cvtph_ps(_mm_loadu_si128(mem_addr as *const __m128i))
    }

    #[cfg(not(target_feature = "f16c"))]
    unsafe fn load(mem_addr: *const bf16) -> Self::Unit {
        let mut tmp = [0.0f32; 8];
        for i in 0..8 {
            tmp[i] = (*mem_addr.add(i)).to_f32();
        }
        _mm256_loadu_ps(tmp.as_ptr())
    }

    unsafe fn vec_add(a: Self::Unit, b: Self::Unit) -> Self::Unit {
        _mm256_add_ps(a, b)
    }

    unsafe fn vec_fma(a: Self::Unit, b: Self::Unit, c: Self::Unit) -> Self::Unit {
        _mm256_add_ps(_mm256_mul_ps(b, c), a)
    }

    #[cfg(target_feature = "f16c")]
    unsafe fn vec_store(mem_addr: *mut bf16, a: Self::Unit) {
        _mm_storeu_si128(mem_addr as *mut __m128i, _mm256_cvtps_ph(a, 0))
    }

    #[cfg(not(target_feature = "f16c"))]
    unsafe fn vec_store(mem_addr: *mut bf16, a: Self::Unit) {
        let mut tmp = [0.0f32; 8];
        _mm256_storeu_ps(tmp.as_mut_ptr(), a);
        for i in 0..8 {
            *mem_addr.add(i) = bf16::from_f32(tmp[i]);
        }
    }

    unsafe fn vec_reduce(mut x: Self::Array, y: *mut f32) {
        let mut offset = ARR >> 1;
        for i in 0..offset {
            x[i] = _mm256_add_ps(x[i], x[offset + i]);
        }
        offset >>= 1;
        for i in 0..offset {
            x[i] = _mm256_add_ps(x[i], x[offset + i]);
        }
        offset >>= 1;
        for i in 0..offset {
            x[i] = _mm256_add_ps(x[i], x[offset + i]);
        }
        let t0 = _mm_add_ps(_mm256_castps256_ps128(x[0]), _mm256_extractf128_ps(x[0], 1));
        let t1 = _mm_hadd_ps(t0, t0);
        *y = _mm_cvtss_f32(_mm_hadd_ps(t1, t1));
    }
}
