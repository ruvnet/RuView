//! SIMD-accelerated sparse matrix-vector multiply.
//!
//! Provides [`spmv_simd`], which dispatches to an architecture-specific
//! implementation when the `simd` feature is enabled, and falls back to a
//! portable scalar loop otherwise.

use crate::types::CsrMatrix;

/// Sparse matrix-vector multiply with optional SIMD acceleration.
///
/// Computes `y = A * x` where `A` is a CSR matrix of `f32` values.
pub fn spmv_simd(matrix: &CsrMatrix<f32>, x: &[f32], y: &mut [f32]) {
    assert_eq!(x.len(), matrix.cols, "x length must equal matrix.cols");
    assert_eq!(y.len(), matrix.rows, "y length must equal matrix.rows");

    #[cfg(all(feature = "simd", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: we have checked for AVX2 support at runtime.
            unsafe {
                spmv_avx2(matrix, x, y);
            }
            return;
        }
    }

    spmv_scalar(matrix, x, y);
}

/// Scalar fallback implementation of SpMV.
pub fn spmv_scalar(matrix: &CsrMatrix<f32>, x: &[f32], y: &mut [f32]) {
    for i in 0..matrix.rows {
        let start = matrix.row_ptr[i];
        let end = matrix.row_ptr[i + 1];
        let mut sum = 0.0f32;
        for idx in start..end {
            let col = matrix.col_indices[idx];
            sum += matrix.values[idx] * x[col];
        }
        y[i] = sum;
    }
}

/// AVX2-accelerated SpMV for x86_64.
///
/// # Safety
///
/// - The caller must ensure AVX2 is supported on the current CPU (checked at
///   runtime via `is_x86_feature_detected!("avx2")` in [`spmv_simd`]).
/// - The caller must ensure `x.len() >= matrix.cols` and
///   `y.len() >= matrix.rows`. These are asserted in [`spmv_simd`] before
///   dispatching here.
/// - The CSR matrix must be structurally valid: `row_ptr[i] <= row_ptr[i+1]`,
///   all `col_indices[j] < matrix.cols`, and `values.len() >= row_ptr[rows]`.
///   Use [`crate::validation::validate_csr_matrix`] before calling the solver
///   to guarantee this.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn spmv_avx2(matrix: &CsrMatrix<f32>, x: &[f32], y: &mut [f32]) {
    use std::arch::x86_64::*;

    for i in 0..matrix.rows {
        let start = matrix.row_ptr[i];
        let end = matrix.row_ptr[i + 1];
        let len = end - start;

        let mut accum = _mm256_setzero_ps();
        let chunks = len / 8;
        let remainder = len % 8;

        for chunk in 0..chunks {
            let base = start + chunk * 8;

            // SAFETY: `base + 7 < end <= values.len()` because
            // `chunk < chunks` implies `base + 8 <= start + chunks * 8 <= end`.
            let vals = _mm256_loadu_ps(matrix.values.as_ptr().add(base));

            let mut x_buf = [0.0f32; 8];
            for k in 0..8 {
                // SAFETY: `base + k < end` so `col_indices[base + k]` is in
                // bounds. `col < matrix.cols <= x.len()` by the CSR structural
                // invariant (enforced by `validate_csr_matrix`).
                let col = *matrix.col_indices.get_unchecked(base + k);
                x_buf[k] = *x.get_unchecked(col);
            }
            let x_vec = _mm256_loadu_ps(x_buf.as_ptr());

            accum = _mm256_add_ps(accum, _mm256_mul_ps(vals, x_vec));
        }

        let mut sum = horizontal_sum_f32x8(accum);

        let tail_start = start + chunks * 8;
        for idx in tail_start..(tail_start + remainder) {
            // SAFETY: `idx < end <= values.len()` and `col < cols <= x.len()`
            // by the same CSR structural invariant.
            let col = *matrix.col_indices.get_unchecked(idx);
            sum += *matrix.values.get_unchecked(idx) * *x.get_unchecked(col);
        }

        // SAFETY: `i < matrix.rows <= y.len()` by the assert in `spmv_simd`.
        *y.get_unchecked_mut(i) = sum;
    }
}

/// Horizontal sum of an AVX2 register (8 x f32 -> 1 x f32).
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn horizontal_sum_f32x8(v: std::arch::x86_64::__m256) -> f32 {
    use std::arch::x86_64::*;

    let hi = _mm256_extractf128_ps(v, 1);
    let lo = _mm256_castps256_ps128(v);
    let sum128 = _mm_add_ps(lo, hi);

    let shuf = _mm_movehdup_ps(sum128);
    let sums = _mm_add_ps(sum128, shuf);
    let shuf2 = _mm_movehl_ps(sums, sums);
    let result = _mm_add_ss(sums, shuf2);
    _mm_cvtss_f32(result)
}

/// Sparse matrix-vector multiply with optional SIMD acceleration for f64.
///
/// Computes `y = A * x` where `A` is a CSR matrix of `f64` values.
pub fn spmv_simd_f64(matrix: &CsrMatrix<f64>, x: &[f64], y: &mut [f64]) {
    assert_eq!(x.len(), matrix.cols, "x length must equal matrix.cols");
    assert_eq!(y.len(), matrix.rows, "y length must equal matrix.rows");

    #[cfg(all(feature = "simd", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe {
                spmv_avx2_f64(matrix, x, y);
            }
            return;
        }
    }

    spmv_scalar_f64(matrix, x, y);
}

/// Scalar fallback for f64 SpMV.
pub fn spmv_scalar_f64(matrix: &CsrMatrix<f64>, x: &[f64], y: &mut [f64]) {
    for i in 0..matrix.rows {
        let start = matrix.row_ptr[i];
        let end = matrix.row_ptr[i + 1];
        let mut sum = 0.0f64;
        for idx in start..end {
            let col = matrix.col_indices[idx];
            sum += matrix.values[idx] * x[col];
        }
        y[i] = sum;
    }
}

#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn spmv_avx2_f64(matrix: &CsrMatrix<f64>, x: &[f64], y: &mut [f64]) {
    use std::arch::x86_64::*;

    for i in 0..matrix.rows {
        let start = matrix.row_ptr[i];
        let end = matrix.row_ptr[i + 1];
        let len = end - start;

        let mut accum = _mm256_setzero_pd();
        let chunks = len / 4;
        let remainder = len % 4;

        for chunk in 0..chunks {
            let base = start + chunk * 4;
            let vals = _mm256_loadu_pd(matrix.values.as_ptr().add(base));

            let mut x_buf = [0.0f64; 4];
            for k in 0..4 {
                let col = *matrix.col_indices.get_unchecked(base + k);
                x_buf[k] = *x.get_unchecked(col);
            }
            let x_vec = _mm256_loadu_pd(x_buf.as_ptr());
            accum = _mm256_add_pd(accum, _mm256_mul_pd(vals, x_vec));
        }

        let mut sum = horizontal_sum_f64x4(accum);

        let tail_start = start + chunks * 4;
        for idx in tail_start..(tail_start + remainder) {
            let col = *matrix.col_indices.get_unchecked(idx);
            sum += *matrix.values.get_unchecked(idx) * *x.get_unchecked(col);
        }

        *y.get_unchecked_mut(i) = sum;
    }
}

#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn horizontal_sum_f64x4(v: std::arch::x86_64::__m256d) -> f64 {
    use std::arch::x86_64::*;
    let hi = _mm256_extractf128_pd(v, 1);
    let lo = _mm256_castpd256_pd128(v);
    let sum128 = _mm_add_pd(lo, hi);
    let hi64 = _mm_unpackhi_pd(sum128, sum128);
    let result = _mm_add_sd(sum128, hi64);
    _mm_cvtsd_f64(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CsrMatrix;

    fn make_test_matrix() -> (CsrMatrix<f32>, Vec<f32>) {
        // [2 0 1]   [1]   [5]
        // [0 3 0] * [2] = [6]
        // [1 0 4]   [3]   [13]
        let mat = CsrMatrix {
            values: vec![2.0, 1.0, 3.0, 1.0, 4.0],
            col_indices: vec![0, 2, 1, 0, 2],
            row_ptr: vec![0, 2, 3, 5],
            rows: 3,
            cols: 3,
        };
        let x = vec![1.0, 2.0, 3.0];
        (mat, x)
    }

    #[test]
    fn scalar_spmv_correctness() {
        let (mat, x) = make_test_matrix();
        let mut y = vec![0.0f32; 3];
        spmv_scalar(&mat, &x, &mut y);
        assert!((y[0] - 5.0).abs() < 1e-6);
        assert!((y[1] - 6.0).abs() < 1e-6);
        assert!((y[2] - 13.0).abs() < 1e-6);
    }

    #[test]
    fn spmv_simd_dispatch() {
        let (mat, x) = make_test_matrix();
        let mut y = vec![0.0f32; 3];
        spmv_simd(&mat, &x, &mut y);
        assert!((y[0] - 5.0).abs() < 1e-6);
        assert!((y[1] - 6.0).abs() < 1e-6);
        assert!((y[2] - 13.0).abs() < 1e-6);
    }

    #[test]
    fn spmv_simd_f64_correctness() {
        let mat = CsrMatrix::<f64> {
            values: vec![2.0, 1.0, 3.0, 1.0, 4.0],
            col_indices: vec![0, 2, 1, 0, 2],
            row_ptr: vec![0, 2, 3, 5],
            rows: 3,
            cols: 3,
        };
        let x = vec![1.0, 2.0, 3.0];
        let mut y = vec![0.0f64; 3];
        spmv_simd_f64(&mat, &x, &mut y);
        assert!((y[0] - 5.0).abs() < 1e-10);
        assert!((y[1] - 6.0).abs() < 1e-10);
        assert!((y[2] - 13.0).abs() < 1e-10);
    }

    #[test]
    fn scalar_spmv_f64_correctness() {
        let mat = CsrMatrix::<f64> {
            values: vec![2.0, 1.0, 3.0, 1.0, 4.0],
            col_indices: vec![0, 2, 1, 0, 2],
            row_ptr: vec![0, 2, 3, 5],
            rows: 3,
            cols: 3,
        };
        let x = vec![1.0, 2.0, 3.0];
        let mut y = vec![0.0f64; 3];
        spmv_scalar_f64(&mat, &x, &mut y);
        assert!((y[0] - 5.0).abs() < 1e-10);
        assert!((y[1] - 6.0).abs() < 1e-10);
        assert!((y[2] - 13.0).abs() < 1e-10);
    }
}
