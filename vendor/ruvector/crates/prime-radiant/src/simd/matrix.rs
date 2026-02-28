//! # SIMD Matrix Operations
//!
//! High-performance matrix operations using SIMD intrinsics.
//! Optimized for small to medium matrices common in coherence computation.
//!
//! ## Matrix Layout
//!
//! All matrices are stored in **row-major** order:
//! - `A[i][j]` is at index `i * cols + j`
//! - This matches Rust's natural 2D array layout
//!
//! ## Supported Operations
//!
//! | Operation | Description | Complexity |
//! |-----------|-------------|------------|
//! | `matmul_simd` | Matrix-matrix multiplication | O(m*k*n) |
//! | `matvec_simd` | Matrix-vector multiplication | O(m*n) |
//! | `transpose_simd` | Matrix transpose | O(m*n) |
//!
//! ## Performance Notes
//!
//! - Uses blocking/tiling for cache-friendly access patterns
//! - Prefetches data for next iteration where beneficial
//! - Falls back to highly optimized scalar code for small matrices

use wide::f32x8;

/// Block size for tiled matrix operations (cache optimization).
const BLOCK_SIZE: usize = 64;

/// Compute matrix-matrix multiplication: C = A * B
///
/// # Arguments
///
/// * `a` - First matrix (m x k), row-major, length = m * k
/// * `b` - Second matrix (k x n), row-major, length = k * n
/// * `c` - Output matrix (m x n), row-major, length = m * n
/// * `m` - Number of rows in A
/// * `k` - Number of columns in A (= rows in B)
/// * `n` - Number of columns in B
///
/// # Panics
///
/// Panics in debug mode if buffer sizes don't match dimensions.
///
/// # Example
///
/// ```rust,ignore
/// use prime_radiant::simd::matrix::matmul_simd;
///
/// // 2x3 * 3x2 = 2x2
/// let a = [1.0, 2.0, 3.0,  4.0, 5.0, 6.0];  // 2x3
/// let b = [1.0, 2.0,  3.0, 4.0,  5.0, 6.0]; // 3x2
/// let mut c = [0.0f32; 4]; // 2x2
///
/// matmul_simd(&a, &b, &mut c, 2, 3, 2);
/// // c = [22, 28, 49, 64]
/// ```
#[inline]
pub fn matmul_simd(a: &[f32], b: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
    debug_assert_eq!(a.len(), m * k, "Matrix A size mismatch");
    debug_assert_eq!(b.len(), k * n, "Matrix B size mismatch");
    debug_assert_eq!(c.len(), m * n, "Matrix C size mismatch");

    // Clear output
    c.fill(0.0);

    // For small matrices, use simple implementation
    if m * n < 256 || k < 8 {
        matmul_scalar(a, b, c, m, k, n);
        return;
    }

    // Blocked/tiled multiplication for cache efficiency
    matmul_blocked(a, b, c, m, k, n);
}

/// Compute matrix-vector multiplication: y = A * x
///
/// # Arguments
///
/// * `a` - Matrix (m x n), row-major
/// * `x` - Input vector (length n)
/// * `y` - Output vector (length m)
/// * `m` - Number of rows
/// * `n` - Number of columns
///
/// # Panics
///
/// Panics in debug mode if buffer sizes don't match dimensions.
#[inline]
pub fn matvec_simd(a: &[f32], x: &[f32], y: &mut [f32], m: usize, n: usize) {
    debug_assert_eq!(a.len(), m * n, "Matrix A size mismatch");
    debug_assert_eq!(x.len(), n, "Vector x size mismatch");
    debug_assert_eq!(y.len(), m, "Vector y size mismatch");

    // For small matrices, use scalar implementation
    if n < 16 {
        matvec_scalar(a, x, y, m, n);
        return;
    }

    // Process each row
    for i in 0..m {
        let row_start = i * n;
        let row = &a[row_start..row_start + n];
        y[i] = dot_product_simd(row, x);
    }
}

/// Transpose a matrix: B = A^T
///
/// # Arguments
///
/// * `a` - Input matrix (m x n), row-major
/// * `b` - Output matrix (n x m), row-major
/// * `m` - Number of rows in A
/// * `n` - Number of columns in A
#[inline]
pub fn transpose_simd(a: &[f32], b: &mut [f32], m: usize, n: usize) {
    debug_assert_eq!(a.len(), m * n);
    debug_assert_eq!(b.len(), m * n);

    // For small matrices, use scalar transpose
    if m < 8 || n < 8 {
        transpose_scalar(a, b, m, n);
        return;
    }

    // Block-based transpose for cache efficiency
    let block = 8;

    for ii in (0..m).step_by(block) {
        for jj in (0..n).step_by(block) {
            // Process block
            let i_end = (ii + block).min(m);
            let j_end = (jj + block).min(n);

            for i in ii..i_end {
                for j in jj..j_end {
                    b[j * m + i] = a[i * n + j];
                }
            }
        }
    }
}

/// Compute outer product: C = a * b^T
///
/// # Arguments
///
/// * `a` - Column vector (length m)
/// * `b` - Row vector (length n)
/// * `c` - Output matrix (m x n), row-major
#[inline]
pub fn outer_product_simd(a: &[f32], b: &[f32], c: &mut [f32]) {
    let m = a.len();
    let n = b.len();
    debug_assert_eq!(c.len(), m * n);

    if n < 16 {
        // Scalar fallback
        for i in 0..m {
            for j in 0..n {
                c[i * n + j] = a[i] * b[j];
            }
        }
        return;
    }

    // SIMD version: each row of C is a[i] * b
    for i in 0..m {
        let scalar = a[i];
        let scalar_vec = f32x8::splat(scalar);
        let row_start = i * n;

        let chunks_b = b.chunks_exact(8);
        let chunks_c = c[row_start..row_start + n].chunks_exact_mut(8);
        let remainder_b = chunks_b.remainder();
        let offset = n - remainder_b.len();

        for (cb, cc) in chunks_b.zip(chunks_c) {
            let vb = load_f32x8(cb);
            let result = vb * scalar_vec;
            store_f32x8(cc, result);
        }

        // Handle remainder
        for (j, &bj) in remainder_b.iter().enumerate() {
            c[row_start + offset + j] = scalar * bj;
        }
    }
}

/// Add two matrices element-wise: C = A + B
#[inline]
pub fn matadd_simd(a: &[f32], b: &[f32], c: &mut [f32]) {
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), c.len());

    let n = a.len();

    if n < 16 {
        for i in 0..n {
            c[i] = a[i] + b[i];
        }
        return;
    }

    let chunks_a = a.chunks_exact(8);
    let chunks_b = b.chunks_exact(8);
    let chunks_c = c.chunks_exact_mut(8);

    let remainder_a = chunks_a.remainder();
    let remainder_b = chunks_b.remainder();
    let offset = n - remainder_a.len();

    for ((ca, cb), cc) in chunks_a.zip(chunks_b).zip(chunks_c) {
        let va = load_f32x8(ca);
        let vb = load_f32x8(cb);
        let result = va + vb;
        store_f32x8(cc, result);
    }

    for (i, (&va, &vb)) in remainder_a.iter().zip(remainder_b.iter()).enumerate() {
        c[offset + i] = va + vb;
    }
}

/// Scale a matrix by a scalar: B = alpha * A
#[inline]
pub fn matscale_simd(a: &[f32], alpha: f32, b: &mut [f32]) {
    debug_assert_eq!(a.len(), b.len());

    let n = a.len();

    if n < 16 {
        for i in 0..n {
            b[i] = alpha * a[i];
        }
        return;
    }

    let alpha_vec = f32x8::splat(alpha);

    let chunks_a = a.chunks_exact(8);
    let chunks_b = b.chunks_exact_mut(8);

    let remainder_a = chunks_a.remainder();
    let offset = n - remainder_a.len();

    for (ca, cb) in chunks_a.zip(chunks_b) {
        let va = load_f32x8(ca);
        let result = va * alpha_vec;
        store_f32x8(cb, result);
    }

    for (i, &va) in remainder_a.iter().enumerate() {
        b[offset + i] = alpha * va;
    }
}

// ============================================================================
// Internal Implementations
// ============================================================================

/// Blocked matrix multiplication for cache efficiency.
fn matmul_blocked(a: &[f32], b: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
    // Use smaller block size for k dimension to keep data in L1 cache
    let bk = BLOCK_SIZE.min(k);
    let bn = BLOCK_SIZE.min(n);

    for kk in (0..k).step_by(bk) {
        let k_end = (kk + bk).min(k);

        for jj in (0..n).step_by(bn) {
            let j_end = (jj + bn).min(n);

            for i in 0..m {
                let c_row = i * n;
                let a_row = i * k;

                // Process this block of the output row
                for kc in kk..k_end {
                    let a_val = a[a_row + kc];
                    let a_vec = f32x8::splat(a_val);
                    let b_row = kc * n;

                    // SIMD inner loop
                    let mut j = jj;
                    while j + 8 <= j_end {
                        let b_chunk = &b[b_row + j..b_row + j + 8];
                        let c_chunk = &mut c[c_row + j..c_row + j + 8];

                        let vb = load_f32x8(b_chunk);
                        let vc = load_f32x8(c_chunk);
                        let result = a_vec.mul_add(vb, vc);
                        store_f32x8(c_chunk, result);

                        j += 8;
                    }

                    // Scalar cleanup
                    while j < j_end {
                        c[c_row + j] += a_val * b[b_row + j];
                        j += 1;
                    }
                }
            }
        }
    }
}

/// Simple scalar matrix multiplication for small matrices.
fn matmul_scalar(a: &[f32], b: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
    for i in 0..m {
        for j in 0..n {
            let mut sum = 0.0f32;
            for kc in 0..k {
                sum += a[i * k + kc] * b[kc * n + j];
            }
            c[i * n + j] = sum;
        }
    }
}

/// Scalar matrix-vector multiplication.
fn matvec_scalar(a: &[f32], x: &[f32], y: &mut [f32], m: usize, n: usize) {
    for i in 0..m {
        let mut sum = 0.0f32;
        let row_start = i * n;
        for j in 0..n {
            sum += a[row_start + j] * x[j];
        }
        y[i] = sum;
    }
}

/// Scalar matrix transpose.
fn transpose_scalar(a: &[f32], b: &mut [f32], m: usize, n: usize) {
    for i in 0..m {
        for j in 0..n {
            b[j * m + i] = a[i * n + j];
        }
    }
}

/// SIMD dot product (copied from vectors module to avoid circular dep).
fn dot_product_simd(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len();

    if n < 16 {
        let mut sum = 0.0f32;
        for i in 0..n {
            sum += a[i] * b[i];
        }
        return sum;
    }

    let chunks_a = a.chunks_exact(8);
    let chunks_b = b.chunks_exact(8);
    let remainder_a = chunks_a.remainder();
    let remainder_b = chunks_b.remainder();

    let mut acc = f32x8::ZERO;

    for (ca, cb) in chunks_a.zip(chunks_b) {
        let va = load_f32x8(ca);
        let vb = load_f32x8(cb);
        acc = va.mul_add(vb, acc);
    }

    let mut sum = acc.reduce_add();

    for (&va, &vb) in remainder_a.iter().zip(remainder_b.iter()) {
        sum += va * vb;
    }

    sum
}

// ============================================================================
// Helper Functions
// ============================================================================

#[inline(always)]
fn load_f32x8(slice: &[f32]) -> f32x8 {
    debug_assert!(slice.len() >= 8);
    // Use try_into for direct memory copy instead of element-by-element
    let arr: [f32; 8] = slice[..8].try_into().unwrap();
    f32x8::from(arr)
}

#[inline(always)]
fn store_f32x8(slice: &mut [f32], v: f32x8) {
    debug_assert!(slice.len() >= 8);
    let arr: [f32; 8] = v.into();
    slice[..8].copy_from_slice(&arr);
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-3;

    fn approx_eq(a: f32, b: f32) -> bool {
        // Use relative error for larger values
        let max_abs = a.abs().max(b.abs());
        if max_abs > 1.0 {
            (a - b).abs() / max_abs < EPSILON
        } else {
            (a - b).abs() < EPSILON
        }
    }

    fn matrices_approx_eq(a: &[f32], b: &[f32]) -> bool {
        a.len() == b.len() && a.iter().zip(b.iter()).all(|(&x, &y)| approx_eq(x, y))
    }

    #[test]
    fn test_matmul_small() {
        // 2x3 * 3x2 = 2x2
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // 2x3
        let b = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // 3x2
        let mut c = [0.0f32; 4]; // 2x2

        matmul_simd(&a, &b, &mut c, 2, 3, 2);

        // Row 0: [1,2,3] * [1,3,5; 2,4,6] = [1*1+2*3+3*5, 1*2+2*4+3*6] = [22, 28]
        // Row 1: [4,5,6] * [1,3,5; 2,4,6] = [4*1+5*3+6*5, 4*2+5*4+6*6] = [49, 64]
        let expected = [22.0, 28.0, 49.0, 64.0];
        assert!(matrices_approx_eq(&c, &expected), "got {:?}", c);
    }

    #[test]
    fn test_matmul_identity() {
        // I * A = A
        let n = 64;
        let mut identity = vec![0.0f32; n * n];
        for i in 0..n {
            identity[i * n + i] = 1.0;
        }

        let a: Vec<f32> = (0..n * n).map(|i| i as f32).collect();
        let mut c = vec![0.0f32; n * n];

        matmul_simd(&identity, &a, &mut c, n, n, n);

        assert!(matrices_approx_eq(&c, &a));
    }

    #[test]
    fn test_matvec_small() {
        // 2x3 matrix * 3-vector
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // 2x3
        let x = [1.0, 2.0, 3.0]; // 3
        let mut y = [0.0f32; 2]; // 2

        matvec_simd(&a, &x, &mut y, 2, 3);

        // y[0] = 1*1 + 2*2 + 3*3 = 14
        // y[1] = 4*1 + 5*2 + 6*3 = 32
        let expected = [14.0, 32.0];
        assert!(matrices_approx_eq(&y, &expected), "got {:?}", y);
    }

    #[test]
    fn test_matvec_large() {
        let m = 64;
        let n = 128;

        let a: Vec<f32> = (0..m * n).map(|i| (i as f32) * 0.01).collect();
        let x: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let mut y_simd = vec![0.0f32; m];
        let mut y_scalar = vec![0.0f32; m];

        matvec_simd(&a, &x, &mut y_simd, m, n);
        matvec_scalar(&a, &x, &mut y_scalar, m, n);

        assert!(matrices_approx_eq(&y_simd, &y_scalar));
    }

    #[test]
    fn test_transpose_small() {
        // 2x3 -> 3x2
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // 2x3
        let mut b = [0.0f32; 6]; // 3x2

        transpose_simd(&a, &mut b, 2, 3);

        // Transposed: [[1,4], [2,5], [3,6]]
        let expected = [1.0, 4.0, 2.0, 5.0, 3.0, 6.0];
        assert_eq!(b, expected);
    }

    #[test]
    fn test_transpose_large() {
        let m = 32;
        let n = 64;

        let a: Vec<f32> = (0..m * n).map(|i| i as f32).collect();
        let mut b = vec![0.0f32; m * n];

        transpose_simd(&a, &mut b, m, n);

        // Verify transpose property
        for i in 0..m {
            for j in 0..n {
                assert!(
                    approx_eq(a[i * n + j], b[j * m + i]),
                    "mismatch at ({}, {})",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn test_outer_product() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0];
        let mut c = [0.0f32; 6];

        outer_product_simd(&a, &b, &mut c);

        // c[i,j] = a[i] * b[j]
        let expected = [4.0, 5.0, 8.0, 10.0, 12.0, 15.0];
        assert!(matrices_approx_eq(&c, &expected));
    }

    #[test]
    fn test_matadd() {
        let a = [1.0, 2.0, 3.0, 4.0];
        let b = [5.0, 6.0, 7.0, 8.0];
        let mut c = [0.0f32; 4];

        matadd_simd(&a, &b, &mut c);

        assert_eq!(c, [6.0, 8.0, 10.0, 12.0]);
    }

    #[test]
    fn test_matscale() {
        let a = [1.0, 2.0, 3.0, 4.0];
        let mut b = [0.0f32; 4];

        matscale_simd(&a, 2.5, &mut b);

        assert!(matrices_approx_eq(&b, &[2.5, 5.0, 7.5, 10.0]));
    }

    #[test]
    fn test_matmul_large() {
        // Test with sizes that exercise the blocked algorithm
        let m = 128;
        let k = 96;
        let n = 64;

        let a: Vec<f32> = (0..m * k).map(|i| (i as f32) * 0.001).collect();
        let b: Vec<f32> = (0..k * n).map(|i| (i as f32) * 0.001).collect();
        let mut c_simd = vec![0.0f32; m * n];
        let mut c_scalar = vec![0.0f32; m * n];

        matmul_simd(&a, &b, &mut c_simd, m, k, n);
        matmul_scalar(&a, &b, &mut c_scalar, m, k, n);

        // Allow slightly more tolerance for larger matrices due to accumulation
        for i in 0..m * n {
            assert!(
                (c_simd[i] - c_scalar[i]).abs() < 0.01,
                "mismatch at {}: {} vs {}",
                i,
                c_simd[i],
                c_scalar[i]
            );
        }
    }
}
