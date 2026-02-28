//! CPU backend with portable SIMD optimizations

use super::Backend;
use crate::config::ActivationType;
use ndarray::Array2;
use std::sync::OnceLock;

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

/// Cached SIMD feature detection for x86_64
#[cfg(target_arch = "x86_64")]
static SIMD_FEATURES: OnceLock<SimdFeatures> = OnceLock::new();

#[cfg(target_arch = "x86_64")]
#[derive(Debug, Clone, Copy)]
struct SimdFeatures {
    has_avx2: bool,
    has_sse41: bool,
    has_fma: bool,
}

#[cfg(target_arch = "x86_64")]
fn get_simd_features() -> SimdFeatures {
    *SIMD_FEATURES.get_or_init(|| SimdFeatures {
        has_avx2: is_x86_feature_detected!("avx2"),
        has_sse41: is_x86_feature_detected!("sse4.1"),
        has_fma: is_x86_feature_detected!("fma"),
    })
}

/// CPU backend using portable SIMD
pub struct CpuBackend;

impl Backend for CpuBackend {
    fn dot_product(&self, a: &[f32], b: &[f32]) -> f32 {
        debug_assert_eq!(a.len(), b.len());

        #[cfg(target_arch = "x86_64")]
        {
            let features = get_simd_features();
            if features.has_avx2 {
                return unsafe { dot_product_avx2(a, b) };
            } else if features.has_sse41 {
                return unsafe { dot_product_sse(a, b) };
            }
            return dot_product_scalar(a, b);
        }

        #[cfg(target_arch = "aarch64")]
        return unsafe { dot_product_neon(a, b) };

        // Fallback scalar
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        dot_product_scalar(a, b)
    }

    fn sparse_matmul(&self, matrix: &Array2<f32>, input: &[f32], rows: &[usize]) -> Vec<f32> {
        let mut output = Vec::with_capacity(rows.len());

        for &row_idx in rows {
            let row = matrix.row(row_idx);
            let dot = self.dot_product(row.as_slice().unwrap(), input);
            output.push(dot);
        }

        output
    }

    fn sparse_matmul_accumulate(
        &self,
        matrix: &Array2<f32>,
        input: &[f32],
        cols: &[usize],
        output: &mut [f32],
    ) {
        for (i, &col_idx) in cols.iter().enumerate() {
            let col = matrix.column(col_idx);
            let scalar = input[i];
            // Column view may not be contiguous, iterate element-by-element
            for (j, &val) in col.iter().enumerate() {
                output[j] += val * scalar;
            }
        }
    }

    fn activation(&self, data: &mut [f32], activation_type: ActivationType) {
        #[cfg(target_arch = "x86_64")]
        let features = get_simd_features();

        match activation_type {
            ActivationType::Relu => {
                #[cfg(target_arch = "x86_64")]
                if features.has_avx2 {
                    return unsafe { relu_avx2(data) };
                }
                relu_scalar(data);
            }
            ActivationType::Gelu => {
                #[cfg(target_arch = "x86_64")]
                if features.has_avx2 {
                    return unsafe { gelu_avx2(data) };
                }
                gelu_scalar(data);
            }
            ActivationType::Silu | ActivationType::Swish => {
                #[cfg(target_arch = "x86_64")]
                if features.has_avx2 {
                    return unsafe { silu_avx2(data) };
                }
                silu_scalar(data);
            }
            ActivationType::Identity => { /* no-op */ }
        }
    }

    fn add(&self, a: &mut [f32], b: &[f32]) {
        debug_assert_eq!(a.len(), b.len());

        #[cfg(target_arch = "x86_64")]
        if get_simd_features().has_avx2 {
            return unsafe { add_avx2(a, b) };
        }

        for (x, y) in a.iter_mut().zip(b.iter()) {
            *x += y;
        }
    }

    fn axpy(&self, a: &mut [f32], b: &[f32], scalar: f32) {
        debug_assert_eq!(a.len(), b.len());

        #[cfg(target_arch = "x86_64")]
        if get_simd_features().has_avx2 {
            return unsafe { axpy_avx2(a, b, scalar) };
        }

        for (x, y) in a.iter_mut().zip(b.iter()) {
            *x += y * scalar;
        }
    }

    fn name(&self) -> &'static str {
        #[cfg(target_arch = "x86_64")]
        {
            let features = get_simd_features();
            if features.has_avx2 {
                return "CPU-AVX2";
            } else if features.has_sse41 {
                return "CPU-SSE4.1";
            }
            return "CPU-Scalar";
        }
        #[cfg(target_arch = "aarch64")]
        return "CPU-NEON";

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        "CPU-Scalar"
    }

    fn simd_width(&self) -> usize {
        #[cfg(target_arch = "x86_64")]
        {
            let features = get_simd_features();
            if features.has_avx2 {
                return 8;
            }
            if features.has_sse41 {
                return 4;
            }
            return 1;
        }
        #[cfg(target_arch = "aarch64")]
        return 4;

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        1
    }
}

// ============ AVX2 Implementations ============

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn dot_product_avx2(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len();
    let chunks = n / 8;

    let mut sum = _mm256_setzero_ps();

    for i in 0..chunks {
        let va = _mm256_loadu_ps(a.as_ptr().add(i * 8));
        let vb = _mm256_loadu_ps(b.as_ptr().add(i * 8));
        sum = _mm256_fmadd_ps(va, vb, sum);
    }

    // Horizontal sum
    let sum128 = _mm_add_ps(_mm256_extractf128_ps(sum, 0), _mm256_extractf128_ps(sum, 1));
    let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
    let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 1));
    let mut result = _mm_cvtss_f32(sum32);

    // Handle remainder
    for i in (chunks * 8)..n {
        result += a[i] * b[i];
    }

    result
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn relu_avx2(data: &mut [f32]) {
    let zero = _mm256_setzero_ps();
    let chunks = data.len() / 8;

    for i in 0..chunks {
        let ptr = data.as_mut_ptr().add(i * 8);
        let v = _mm256_loadu_ps(ptr);
        let result = _mm256_max_ps(v, zero);
        _mm256_storeu_ps(ptr, result);
    }

    // Handle remainder
    for i in (chunks * 8)..data.len() {
        data[i] = data[i].max(0.0);
    }
}

/// SIMD GELU using polynomial approximation
/// GELU(x) ≈ 0.5 * x * (1 + tanh(sqrt(2/π) * (x + 0.044715 * x³)))
/// Using fast tanh approximation for SIMD
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2", enable = "fma")]
unsafe fn gelu_avx2(data: &mut [f32]) {
    let chunks = data.len() / 8;

    // Constants for GELU approximation
    let half = _mm256_set1_ps(0.5);
    let one = _mm256_set1_ps(1.0);
    let sqrt_2_over_pi = _mm256_set1_ps(0.7978845608); // sqrt(2/π)
    let coef = _mm256_set1_ps(0.044715);

    // Constants for fast tanh approximation: tanh(x) ≈ x * (27 + x²) / (27 + 9x²)
    let c27 = _mm256_set1_ps(27.0);
    let c9 = _mm256_set1_ps(9.0);

    for i in 0..chunks {
        let ptr = data.as_mut_ptr().add(i * 8);
        let x = _mm256_loadu_ps(ptr);

        // x³
        let x2 = _mm256_mul_ps(x, x);
        let x3 = _mm256_mul_ps(x2, x);

        // inner = sqrt(2/π) * (x + 0.044715 * x³)
        let inner = _mm256_mul_ps(sqrt_2_over_pi, _mm256_fmadd_ps(coef, x3, x));

        // Fast tanh approximation
        let inner2 = _mm256_mul_ps(inner, inner);
        let num = _mm256_fmadd_ps(inner2, one, c27); // 27 + inner²
        let den = _mm256_fmadd_ps(inner2, c9, c27); // 27 + 9*inner²
        let tanh_approx = _mm256_mul_ps(inner, _mm256_div_ps(num, den));

        // 0.5 * x * (1 + tanh)
        let result = _mm256_mul_ps(half, _mm256_mul_ps(x, _mm256_add_ps(one, tanh_approx)));
        _mm256_storeu_ps(ptr, result);
    }

    // Handle remainder with scalar
    for i in (chunks * 8)..data.len() {
        let x = data[i];
        let x3 = x * x * x;
        let inner = 0.7978845608 * (x + 0.044715 * x3);
        data[i] = 0.5 * x * (1.0 + inner.tanh());
    }
}

/// SIMD SiLU (Swish) using fast sigmoid approximation
/// SiLU(x) = x * sigmoid(x) = x / (1 + exp(-x))
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2", enable = "fma")]
unsafe fn silu_avx2(data: &mut [f32]) {
    let chunks = data.len() / 8;

    // For sigmoid, use: 1/(1+e^-x) ≈ 0.5 + 0.5*tanh(x/2)
    let half = _mm256_set1_ps(0.5);
    let c27 = _mm256_set1_ps(27.0);
    let c9 = _mm256_set1_ps(9.0);
    let one = _mm256_set1_ps(1.0);

    for i in 0..chunks {
        let ptr = data.as_mut_ptr().add(i * 8);
        let x = _mm256_loadu_ps(ptr);

        // Use sigmoid(x) = 0.5 + 0.5 * tanh(x/2)
        let x_half = _mm256_mul_ps(x, half);

        // Fast tanh(x/2)
        let xh2 = _mm256_mul_ps(x_half, x_half);
        let num = _mm256_fmadd_ps(xh2, one, c27);
        let den = _mm256_fmadd_ps(xh2, c9, c27);
        let tanh_approx = _mm256_mul_ps(x_half, _mm256_div_ps(num, den));

        // sigmoid = 0.5 + 0.5 * tanh
        let sigmoid = _mm256_fmadd_ps(half, tanh_approx, half);

        // silu = x * sigmoid
        let result = _mm256_mul_ps(x, sigmoid);
        _mm256_storeu_ps(ptr, result);
    }

    // Handle remainder with scalar
    for i in (chunks * 8)..data.len() {
        let x = data[i];
        data[i] = x / (1.0 + (-x).exp());
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn add_avx2(a: &mut [f32], b: &[f32]) {
    let chunks = a.len() / 8;

    for i in 0..chunks {
        let pa = a.as_mut_ptr().add(i * 8);
        let pb = b.as_ptr().add(i * 8);
        let va = _mm256_loadu_ps(pa);
        let vb = _mm256_loadu_ps(pb);
        _mm256_storeu_ps(pa, _mm256_add_ps(va, vb));
    }

    for i in (chunks * 8)..a.len() {
        a[i] += b[i];
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn axpy_avx2(a: &mut [f32], b: &[f32], scalar: f32) {
    let vs = _mm256_set1_ps(scalar);
    let chunks = a.len() / 8;

    for i in 0..chunks {
        let pa = a.as_mut_ptr().add(i * 8);
        let pb = b.as_ptr().add(i * 8);
        let va = _mm256_loadu_ps(pa);
        let vb = _mm256_loadu_ps(pb);
        let result = _mm256_fmadd_ps(vb, vs, va);
        _mm256_storeu_ps(pa, result);
    }

    for i in (chunks * 8)..a.len() {
        a[i] += b[i] * scalar;
    }
}

// ============ SSE4.1 Implementations ============

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn dot_product_sse(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len();
    let chunks = n / 4;

    let mut sum = _mm_setzero_ps();

    for i in 0..chunks {
        let va = _mm_loadu_ps(a.as_ptr().add(i * 4));
        let vb = _mm_loadu_ps(b.as_ptr().add(i * 4));
        sum = _mm_add_ps(sum, _mm_mul_ps(va, vb));
    }

    // Horizontal sum
    let sum2 = _mm_add_ps(sum, _mm_movehl_ps(sum, sum));
    let sum1 = _mm_add_ss(sum2, _mm_shuffle_ps(sum2, sum2, 1));
    let mut result = _mm_cvtss_f32(sum1);

    for i in (chunks * 4)..n {
        result += a[i] * b[i];
    }

    result
}

// ============ NEON Implementations (ARM) ============

#[cfg(target_arch = "aarch64")]
unsafe fn dot_product_neon(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len();
    let chunks = n / 4;

    let mut sum = vdupq_n_f32(0.0);

    for i in 0..chunks {
        let va = vld1q_f32(a.as_ptr().add(i * 4));
        let vb = vld1q_f32(b.as_ptr().add(i * 4));
        sum = vfmaq_f32(sum, va, vb);
    }

    // Horizontal sum
    let mut result = vaddvq_f32(sum);

    for i in (chunks * 4)..n {
        result += a[i] * b[i];
    }

    result
}

// ============ Scalar Fallbacks ============

fn dot_product_scalar(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn relu_scalar(data: &mut [f32]) {
    for x in data.iter_mut() {
        *x = x.max(0.0);
    }
}

fn gelu_scalar(data: &mut [f32]) {
    const SQRT_2_OVER_PI: f32 = 0.7978845608;
    const GELU_COEF: f32 = 0.044715;

    for x in data.iter_mut() {
        let x3 = *x * *x * *x;
        let inner = SQRT_2_OVER_PI * (*x + GELU_COEF * x3);
        *x = 0.5 * *x * (1.0 + inner.tanh());
    }
}

fn silu_scalar(data: &mut [f32]) {
    for x in data.iter_mut() {
        *x = *x / (1.0 + (-*x).exp());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dot_product() {
        let backend = CpuBackend;
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![2.0, 3.0, 4.0, 5.0];
        let result = backend.dot_product(&a, &b);
        assert!((result - 40.0).abs() < 1e-5);
    }

    #[test]
    fn test_relu() {
        let backend = CpuBackend;
        let mut data = vec![-1.0, 0.0, 1.0, 2.0];
        backend.activation(&mut data, ActivationType::Relu);
        assert_eq!(data, vec![0.0, 0.0, 1.0, 2.0]);
    }

    #[test]
    fn test_add() {
        let backend = CpuBackend;
        let mut a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0, 8.0];
        backend.add(&mut a, &b);
        assert_eq!(a, vec![6.0, 8.0, 10.0, 12.0]);
    }

    #[test]
    fn test_axpy() {
        let backend = CpuBackend;
        let mut a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![1.0, 1.0, 1.0, 1.0];
        backend.axpy(&mut a, &b, 2.0);
        assert_eq!(a, vec![3.0, 4.0, 5.0, 6.0]);
    }
}
