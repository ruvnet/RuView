//! Apple Accelerate Framework Integration for GEMV
//!
//! Provides high-performance matrix-vector multiplication using Apple's
//! Accelerate framework BLAS implementation. On Apple Silicon, this achieves
//! significantly higher throughput than hand-written NEON kernels due to:
//!
//! - Apple's proprietary AMX (Apple Matrix Extensions) coprocessor
//! - Highly optimized microarchitecture-specific implementations
//! - Multi-core parallelization built into the framework
//!
//! ## Performance Characteristics (M4 Pro)
//!
//! | Operation | NEON Kernel | Accelerate | Speedup |
//! |-----------|-------------|------------|---------|
//! | GEMV 4096x4096 | ~35 GFLOPS | ~80+ GFLOPS | ~2.2x |
//! | GEMV 8192x8192 | ~32 GFLOPS | ~85+ GFLOPS | ~2.7x |
//!
//! ## Usage
//!
//! The Accelerate backend is automatically selected when:
//! 1. Running on macOS
//! 2. The `accelerate` feature is enabled
//! 3. Matrix dimensions meet minimum thresholds
//!
//! ```rust,ignore
//! use ruvllm::kernels::gemv_accelerate;
//!
//! let a = vec![1.0f32; 4096 * 4096];
//! let x = vec![1.0f32; 4096];
//! let mut y = vec![0.0f32; 4096];
//!
//! // Uses Accelerate framework for optimal performance
//! gemv_accelerate(&a, &x, &mut y, 4096, 4096, MatrixLayout::RowMajor);
//! ```
//!
//! ## Feature Flag
//!
//! Enable with the `accelerate` feature in `Cargo.toml`:
//! ```toml
//! ruvllm = { version = "0.1", features = ["accelerate"] }
//! ```

// ============================================================================
// FFI Bindings to Apple Accelerate Framework
// ============================================================================

/// CBLAS matrix storage order
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CblasOrder {
    /// Row-major storage (C-style)
    RowMajor = 101,
    /// Column-major storage (Fortran-style)
    ColMajor = 102,
}

/// CBLAS matrix transpose operation
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CblasTranspose {
    /// No transpose
    NoTrans = 111,
    /// Transpose
    Trans = 112,
    /// Conjugate transpose (for complex types)
    ConjTrans = 113,
}

/// Matrix layout for public API
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatrixLayout {
    /// Row-major storage (C-style) - default for Rust arrays
    #[default]
    RowMajor,
    /// Column-major storage (Fortran-style)
    ColMajor,
}

impl From<MatrixLayout> for CblasOrder {
    fn from(layout: MatrixLayout) -> Self {
        match layout {
            MatrixLayout::RowMajor => CblasOrder::RowMajor,
            MatrixLayout::ColMajor => CblasOrder::ColMajor,
        }
    }
}

// Link against the Accelerate framework on macOS
#[cfg(all(target_os = "macos", feature = "accelerate"))]
#[link(name = "Accelerate", kind = "framework")]
extern "C" {
    /// Single-precision general matrix-vector multiplication
    ///
    /// Computes: y = alpha * op(A) * x + beta * y
    ///
    /// Where op(A) is either A or A^T depending on `trans`.
    ///
    /// # Parameters
    /// - `order`: Row-major (101) or column-major (102)
    /// - `trans`: No transpose (111) or transpose (112)
    /// - `m`: Number of rows of matrix A
    /// - `n`: Number of columns of matrix A
    /// - `alpha`: Scalar multiplier for A * x
    /// - `a`: Pointer to matrix A
    /// - `lda`: Leading dimension of A (typically n for row-major)
    /// - `x`: Pointer to vector x
    /// - `incx`: Increment for x (typically 1)
    /// - `beta`: Scalar multiplier for y
    /// - `y`: Pointer to output vector y
    /// - `incy`: Increment for y (typically 1)
    fn cblas_sgemv(
        order: i32,
        trans: i32,
        m: i32,
        n: i32,
        alpha: f32,
        a: *const f32,
        lda: i32,
        x: *const f32,
        incx: i32,
        beta: f32,
        y: *mut f32,
        incy: i32,
    );

    /// Single-precision general matrix-matrix multiplication
    ///
    /// Computes: C = alpha * op(A) * op(B) + beta * C
    fn cblas_sgemm(
        order: i32,
        transa: i32,
        transb: i32,
        m: i32,
        n: i32,
        k: i32,
        alpha: f32,
        a: *const f32,
        lda: i32,
        b: *const f32,
        ldb: i32,
        beta: f32,
        c: *mut f32,
        ldc: i32,
    );

    /// Single-precision dot product
    fn cblas_sdot(n: i32, x: *const f32, incx: i32, y: *const f32, incy: i32) -> f32;

    /// Single-precision vector scaling: x = alpha * x
    fn cblas_sscal(n: i32, alpha: f32, x: *mut f32, incx: i32);

    /// Single-precision axpy: y = alpha * x + y
    fn cblas_saxpy(n: i32, alpha: f32, x: *const f32, incx: i32, y: *mut f32, incy: i32);
}

// ============================================================================
// Public API - Accelerate GEMV
// ============================================================================

/// Minimum dimension for Accelerate to be beneficial over NEON
/// Below this threshold, NEON overhead is lower due to function call cost
const ACCELERATE_MIN_DIM: usize = 256;

/// Minimum total operations (m * n) for Accelerate
const ACCELERATE_MIN_OPS: usize = 65536; // 256 * 256

/// Check if Accelerate framework is available
#[inline(always)]
pub fn is_accelerate_available() -> bool {
    #[cfg(all(target_os = "macos", feature = "accelerate"))]
    {
        true
    }
    #[cfg(not(all(target_os = "macos", feature = "accelerate")))]
    {
        false
    }
}

/// Check if Accelerate should be used for given dimensions
///
/// Returns true if:
/// 1. Accelerate is available
/// 2. Matrix dimensions are large enough to benefit
#[inline(always)]
pub fn should_use_accelerate(m: usize, n: usize) -> bool {
    is_accelerate_available()
        && m >= ACCELERATE_MIN_DIM
        && n >= ACCELERATE_MIN_DIM
        && m * n >= ACCELERATE_MIN_OPS
}

/// General Matrix-Vector multiplication using Apple Accelerate
///
/// Computes: y = A * x
///
/// Uses Apple's BLAS implementation which leverages the AMX coprocessor
/// on Apple Silicon for maximum throughput.
///
/// # Arguments
/// * `a` - Matrix A (m x n), in specified layout
/// * `x` - Vector x (n,)
/// * `y` - Output vector y (m,), modified in-place
/// * `m` - Number of rows in A
/// * `n` - Number of columns in A (length of x)
/// * `layout` - Matrix storage order (RowMajor or ColMajor)
///
/// # Performance
/// On M4 Pro: ~80+ GFLOPS for large matrices (2x+ vs NEON)
///
/// # Panics
/// Panics if dimensions don't match or if not on macOS with accelerate feature
///
/// # Example
/// ```rust,ignore
/// use ruvllm::kernels::accelerate::{gemv_accelerate, MatrixLayout};
///
/// let a = vec![1.0f32; 4096 * 4096];
/// let x = vec![1.0f32; 4096];
/// let mut y = vec![0.0f32; 4096];
///
/// gemv_accelerate(&a, &x, &mut y, 4096, 4096, MatrixLayout::RowMajor);
/// ```
#[cfg(all(target_os = "macos", feature = "accelerate"))]
pub fn gemv_accelerate(
    a: &[f32],
    x: &[f32],
    y: &mut [f32],
    m: usize,
    n: usize,
    layout: MatrixLayout,
) {
    debug_assert_eq!(
        a.len(),
        m * n,
        "Matrix A size mismatch: expected {}, got {}",
        m * n,
        a.len()
    );
    debug_assert_eq!(
        x.len(),
        n,
        "Vector x size mismatch: expected {}, got {}",
        n,
        x.len()
    );
    debug_assert_eq!(
        y.len(),
        m,
        "Vector y size mismatch: expected {}, got {}",
        m,
        y.len()
    );

    // SECURITY FIX (H-005): Bounds check before i32 cast to prevent overflow
    // BLAS uses i32 for dimensions, so we must ensure values fit
    assert!(
        m <= i32::MAX as usize,
        "Matrix dimension m={} exceeds i32::MAX for BLAS",
        m
    );
    assert!(
        n <= i32::MAX as usize,
        "Matrix dimension n={} exceeds i32::MAX for BLAS",
        n
    );

    unsafe {
        gemv_accelerate_unchecked(a, x, y, m, n, layout);
    }
}

/// Unchecked GEMV using Accelerate
///
/// # Safety
/// Caller must ensure:
/// - `a.len() >= m * n`
/// - `x.len() >= n`
/// - `y.len() >= m`
/// - Pointers are properly aligned
#[cfg(all(target_os = "macos", feature = "accelerate"))]
#[inline(always)]
pub unsafe fn gemv_accelerate_unchecked(
    a: &[f32],
    x: &[f32],
    y: &mut [f32],
    m: usize,
    n: usize,
    layout: MatrixLayout,
) {
    let order = CblasOrder::from(layout) as i32;
    let trans = CblasTranspose::NoTrans as i32;

    // For row-major: A is m x n, lda = n
    // For col-major: A is m x n, lda = m
    let lda = match layout {
        MatrixLayout::RowMajor => n as i32,
        MatrixLayout::ColMajor => m as i32,
    };

    cblas_sgemv(
        order,
        trans,
        m as i32,
        n as i32,
        1.0, // alpha = 1
        a.as_ptr(),
        lda,
        x.as_ptr(),
        1,   // incx = 1
        0.0, // beta = 0 (overwrite y)
        y.as_mut_ptr(),
        1, // incy = 1
    );
}

/// GEMV with transpose using Accelerate
///
/// Computes: y = A^T * x
///
/// # Arguments
/// * `a` - Matrix A (m x n), in specified layout
/// * `x` - Vector x (m,) - note: length is m due to transpose
/// * `y` - Output vector y (n,), modified in-place
/// * `m` - Number of rows in A
/// * `n` - Number of columns in A
/// * `layout` - Matrix storage order
#[cfg(all(target_os = "macos", feature = "accelerate"))]
pub fn gemv_transpose_accelerate(
    a: &[f32],
    x: &[f32],
    y: &mut [f32],
    m: usize,
    n: usize,
    layout: MatrixLayout,
) {
    debug_assert_eq!(a.len(), m * n);
    debug_assert_eq!(x.len(), m); // Note: x length is m for transpose
    debug_assert_eq!(y.len(), n); // Note: y length is n for transpose

    // SECURITY FIX (H-005): Bounds check before i32 cast to prevent overflow
    assert!(
        m <= i32::MAX as usize,
        "Matrix dimension m={} exceeds i32::MAX for BLAS",
        m
    );
    assert!(
        n <= i32::MAX as usize,
        "Matrix dimension n={} exceeds i32::MAX for BLAS",
        n
    );

    unsafe {
        let order = CblasOrder::from(layout) as i32;
        let trans = CblasTranspose::Trans as i32;

        let lda = match layout {
            MatrixLayout::RowMajor => n as i32,
            MatrixLayout::ColMajor => m as i32,
        };

        cblas_sgemv(
            order,
            trans,
            m as i32,
            n as i32,
            1.0,
            a.as_ptr(),
            lda,
            x.as_ptr(),
            1,
            0.0,
            y.as_mut_ptr(),
            1,
        );
    }
}

/// GEMV with alpha and beta scaling using Accelerate
///
/// Computes: y = alpha * A * x + beta * y
///
/// This is the full BLAS sgemv operation with scaling factors.
///
/// # Arguments
/// * `a` - Matrix A (m x n)
/// * `x` - Vector x (n,)
/// * `y` - Vector y (m,), updated in-place
/// * `m` - Number of rows in A
/// * `n` - Number of columns in A
/// * `alpha` - Scalar multiplier for A * x
/// * `beta` - Scalar multiplier for existing y values
/// * `layout` - Matrix storage order
#[cfg(all(target_os = "macos", feature = "accelerate"))]
pub fn gemv_scaled_accelerate(
    a: &[f32],
    x: &[f32],
    y: &mut [f32],
    m: usize,
    n: usize,
    alpha: f32,
    beta: f32,
    layout: MatrixLayout,
) {
    debug_assert_eq!(a.len(), m * n);
    debug_assert_eq!(x.len(), n);
    debug_assert_eq!(y.len(), m);

    // SECURITY FIX (H-005): Bounds check before i32 cast to prevent overflow
    assert!(
        m <= i32::MAX as usize,
        "Matrix dimension m={} exceeds i32::MAX for BLAS",
        m
    );
    assert!(
        n <= i32::MAX as usize,
        "Matrix dimension n={} exceeds i32::MAX for BLAS",
        n
    );

    unsafe {
        let order = CblasOrder::from(layout) as i32;
        let trans = CblasTranspose::NoTrans as i32;

        let lda = match layout {
            MatrixLayout::RowMajor => n as i32,
            MatrixLayout::ColMajor => m as i32,
        };

        cblas_sgemv(
            order,
            trans,
            m as i32,
            n as i32,
            alpha,
            a.as_ptr(),
            lda,
            x.as_ptr(),
            1,
            beta,
            y.as_mut_ptr(),
            1,
        );
    }
}

// ============================================================================
// Public API - Accelerate GEMM
// ============================================================================

/// General Matrix-Matrix multiplication using Apple Accelerate
///
/// Computes: C = A * B
///
/// # Arguments
/// * `a` - Matrix A (m x k), row-major
/// * `b` - Matrix B (k x n), row-major
/// * `c` - Output matrix C (m x n), row-major, modified in-place
/// * `m` - Number of rows in A and C
/// * `k` - Number of columns in A, rows in B
/// * `n` - Number of columns in B and C
#[cfg(all(target_os = "macos", feature = "accelerate"))]
pub fn gemm_accelerate(a: &[f32], b: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
    debug_assert_eq!(a.len(), m * k);
    debug_assert_eq!(b.len(), k * n);
    debug_assert_eq!(c.len(), m * n);

    // SECURITY FIX (H-005): Bounds check before i32 cast to prevent overflow
    assert!(
        m <= i32::MAX as usize,
        "Matrix dimension m={} exceeds i32::MAX for BLAS",
        m
    );
    assert!(
        k <= i32::MAX as usize,
        "Matrix dimension k={} exceeds i32::MAX for BLAS",
        k
    );
    assert!(
        n <= i32::MAX as usize,
        "Matrix dimension n={} exceeds i32::MAX for BLAS",
        n
    );

    unsafe {
        cblas_sgemm(
            CblasOrder::RowMajor as i32,
            CblasTranspose::NoTrans as i32,
            CblasTranspose::NoTrans as i32,
            m as i32,
            n as i32,
            k as i32,
            1.0, // alpha
            a.as_ptr(),
            k as i32, // lda
            b.as_ptr(),
            n as i32, // ldb
            0.0,      // beta
            c.as_mut_ptr(),
            n as i32, // ldc
        );
    }
}

// ============================================================================
// Additional BLAS Operations
// ============================================================================

/// Single-precision dot product using Accelerate
///
/// Computes: result = x . y
#[cfg(all(target_os = "macos", feature = "accelerate"))]
#[inline]
pub fn dot_accelerate(x: &[f32], y: &[f32]) -> f32 {
    debug_assert_eq!(x.len(), y.len());
    unsafe { cblas_sdot(x.len() as i32, x.as_ptr(), 1, y.as_ptr(), 1) }
}

/// Scale vector in-place using Accelerate
///
/// Computes: x = alpha * x
#[cfg(all(target_os = "macos", feature = "accelerate"))]
#[inline]
pub fn scal_accelerate(x: &mut [f32], alpha: f32) {
    unsafe { cblas_sscal(x.len() as i32, alpha, x.as_mut_ptr(), 1) }
}

/// Vector addition with scaling using Accelerate
///
/// Computes: y = alpha * x + y
#[cfg(all(target_os = "macos", feature = "accelerate"))]
#[inline]
pub fn axpy_accelerate(x: &[f32], y: &mut [f32], alpha: f32) {
    debug_assert_eq!(x.len(), y.len());
    unsafe { cblas_saxpy(x.len() as i32, alpha, x.as_ptr(), 1, y.as_mut_ptr(), 1) }
}

// ============================================================================
// Fallback implementations for non-macOS platforms
// ============================================================================

#[cfg(not(all(target_os = "macos", feature = "accelerate")))]
pub fn gemv_accelerate(
    _a: &[f32],
    _x: &[f32],
    _y: &mut [f32],
    _m: usize,
    _n: usize,
    _layout: MatrixLayout,
) {
    panic!("Accelerate framework is only available on macOS with 'accelerate' feature enabled");
}

#[cfg(not(all(target_os = "macos", feature = "accelerate")))]
pub unsafe fn gemv_accelerate_unchecked(
    _a: &[f32],
    _x: &[f32],
    _y: &mut [f32],
    _m: usize,
    _n: usize,
    _layout: MatrixLayout,
) {
    panic!("Accelerate framework is only available on macOS with 'accelerate' feature enabled");
}

#[cfg(not(all(target_os = "macos", feature = "accelerate")))]
pub fn gemv_transpose_accelerate(
    _a: &[f32],
    _x: &[f32],
    _y: &mut [f32],
    _m: usize,
    _n: usize,
    _layout: MatrixLayout,
) {
    panic!("Accelerate framework is only available on macOS with 'accelerate' feature enabled");
}

#[cfg(not(all(target_os = "macos", feature = "accelerate")))]
pub fn gemv_scaled_accelerate(
    _a: &[f32],
    _x: &[f32],
    _y: &mut [f32],
    _m: usize,
    _n: usize,
    _alpha: f32,
    _beta: f32,
    _layout: MatrixLayout,
) {
    panic!("Accelerate framework is only available on macOS with 'accelerate' feature enabled");
}

#[cfg(not(all(target_os = "macos", feature = "accelerate")))]
pub fn gemm_accelerate(_a: &[f32], _b: &[f32], _c: &mut [f32], _m: usize, _k: usize, _n: usize) {
    panic!("Accelerate framework is only available on macOS with 'accelerate' feature enabled");
}

#[cfg(not(all(target_os = "macos", feature = "accelerate")))]
pub fn dot_accelerate(_x: &[f32], _y: &[f32]) -> f32 {
    panic!("Accelerate framework is only available on macOS with 'accelerate' feature enabled");
}

#[cfg(not(all(target_os = "macos", feature = "accelerate")))]
pub fn scal_accelerate(_x: &mut [f32], _alpha: f32) {
    panic!("Accelerate framework is only available on macOS with 'accelerate' feature enabled");
}

#[cfg(not(all(target_os = "macos", feature = "accelerate")))]
pub fn axpy_accelerate(_x: &[f32], _y: &mut [f32], _alpha: f32) {
    panic!("Accelerate framework is only available on macOS with 'accelerate' feature enabled");
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accelerate_availability() {
        #[cfg(all(target_os = "macos", feature = "accelerate"))]
        assert!(is_accelerate_available());

        #[cfg(not(all(target_os = "macos", feature = "accelerate")))]
        assert!(!is_accelerate_available());
    }

    #[test]
    fn test_should_use_accelerate_thresholds() {
        // Below threshold
        assert!(!should_use_accelerate(128, 128));
        assert!(!should_use_accelerate(255, 256));

        // At/above threshold (only true on macOS with feature)
        #[cfg(all(target_os = "macos", feature = "accelerate"))]
        {
            assert!(should_use_accelerate(256, 256));
            assert!(should_use_accelerate(4096, 4096));
        }

        #[cfg(not(all(target_os = "macos", feature = "accelerate")))]
        {
            assert!(!should_use_accelerate(256, 256));
            assert!(!should_use_accelerate(4096, 4096));
        }
    }

    #[cfg(all(target_os = "macos", feature = "accelerate"))]
    #[test]
    fn test_gemv_accelerate_correctness() {
        // Simple 2x3 matrix test
        // A = [[1, 2, 3],
        //      [4, 5, 6]]
        // x = [1, 1, 1]
        // y = A * x = [6, 15]
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let x = vec![1.0, 1.0, 1.0];
        let mut y = vec![0.0, 0.0];

        gemv_accelerate(&a, &x, &mut y, 2, 3, MatrixLayout::RowMajor);

        assert!((y[0] - 6.0).abs() < 1e-5);
        assert!((y[1] - 15.0).abs() < 1e-5);
    }

    #[cfg(all(target_os = "macos", feature = "accelerate"))]
    #[test]
    fn test_gemv_transpose_correctness() {
        // A = [[1, 2, 3],
        //      [4, 5, 6]]
        // x = [1, 1]
        // y = A^T * x = [5, 7, 9]
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let x = vec![1.0, 1.0];
        let mut y = vec![0.0, 0.0, 0.0];

        gemv_transpose_accelerate(&a, &x, &mut y, 2, 3, MatrixLayout::RowMajor);

        assert!((y[0] - 5.0).abs() < 1e-5);
        assert!((y[1] - 7.0).abs() < 1e-5);
        assert!((y[2] - 9.0).abs() < 1e-5);
    }

    #[cfg(all(target_os = "macos", feature = "accelerate"))]
    #[test]
    fn test_gemv_scaled_correctness() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let x = vec![1.0, 1.0, 1.0];
        let mut y = vec![1.0, 2.0]; // Initial values

        // y = 2 * A * x + 3 * y
        // y = 2 * [6, 15] + 3 * [1, 2] = [12, 30] + [3, 6] = [15, 36]
        gemv_scaled_accelerate(&a, &x, &mut y, 2, 3, 2.0, 3.0, MatrixLayout::RowMajor);

        assert!((y[0] - 15.0).abs() < 1e-5);
        assert!((y[1] - 36.0).abs() < 1e-5);
    }

    #[cfg(all(target_os = "macos", feature = "accelerate"))]
    #[test]
    fn test_gemm_accelerate_correctness() {
        // A = [[1, 2],
        //      [3, 4]]
        // B = [[5, 6],
        //      [7, 8]]
        // C = A * B = [[19, 22],
        //              [43, 50]]
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![5.0, 6.0, 7.0, 8.0];
        let mut c = vec![0.0; 4];

        gemm_accelerate(&a, &b, &mut c, 2, 2, 2);

        assert!((c[0] - 19.0).abs() < 1e-5);
        assert!((c[1] - 22.0).abs() < 1e-5);
        assert!((c[2] - 43.0).abs() < 1e-5);
        assert!((c[3] - 50.0).abs() < 1e-5);
    }

    #[cfg(all(target_os = "macos", feature = "accelerate"))]
    #[test]
    fn test_dot_accelerate_correctness() {
        let x = vec![1.0, 2.0, 3.0];
        let y = vec![4.0, 5.0, 6.0];

        let result = dot_accelerate(&x, &y);

        // 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32
        assert!((result - 32.0).abs() < 1e-5);
    }

    #[cfg(all(target_os = "macos", feature = "accelerate"))]
    #[test]
    fn test_scal_accelerate_correctness() {
        let mut x = vec![1.0, 2.0, 3.0];

        scal_accelerate(&mut x, 2.0);

        assert!((x[0] - 2.0).abs() < 1e-5);
        assert!((x[1] - 4.0).abs() < 1e-5);
        assert!((x[2] - 6.0).abs() < 1e-5);
    }

    #[cfg(all(target_os = "macos", feature = "accelerate"))]
    #[test]
    fn test_axpy_accelerate_correctness() {
        let x = vec![1.0, 2.0, 3.0];
        let mut y = vec![4.0, 5.0, 6.0];

        // y = 2 * x + y = [2, 4, 6] + [4, 5, 6] = [6, 9, 12]
        axpy_accelerate(&x, &mut y, 2.0);

        assert!((y[0] - 6.0).abs() < 1e-5);
        assert!((y[1] - 9.0).abs() < 1e-5);
        assert!((y[2] - 12.0).abs() < 1e-5);
    }

    #[cfg(all(target_os = "macos", feature = "accelerate"))]
    #[test]
    fn test_gemv_large_matrix() {
        // Test with a larger matrix to verify performance path
        let m = 512;
        let n = 512;
        let a: Vec<f32> = (0..m * n).map(|i| (i % 10) as f32 * 0.1).collect();
        let x: Vec<f32> = vec![1.0; n];
        let mut y = vec![0.0; m];

        gemv_accelerate(&a, &x, &mut y, m, n, MatrixLayout::RowMajor);

        // Verify non-zero results
        assert!(y.iter().any(|&v| v != 0.0));
    }

    #[cfg(all(target_os = "macos", feature = "accelerate"))]
    #[test]
    fn test_col_major_layout() {
        // Test column-major layout
        // A stored col-major: column 0 = [1, 4], column 1 = [2, 5], column 2 = [3, 6]
        // Storage: [1, 4, 2, 5, 3, 6]
        // Logical matrix (2x3):
        // [[1, 2, 3],
        //  [4, 5, 6]]
        let a = vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]; // Column-major storage
        let x = vec![1.0, 1.0, 1.0];
        let mut y = vec![0.0, 0.0];

        gemv_accelerate(&a, &x, &mut y, 2, 3, MatrixLayout::ColMajor);

        assert!((y[0] - 6.0).abs() < 1e-5);
        assert!((y[1] - 15.0).abs() < 1e-5);
    }
}
