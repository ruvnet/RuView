//! Comprehensive input validation for solver operations.
//!
//! All validation functions run eagerly before any computation begins, ensuring
//! callers receive clear diagnostics instead of mysterious numerical failures or
//! resource exhaustion. Every public function returns [`ValidationError`] on
//! failure, which converts into [`SolverError::InvalidInput`] via `From`.
//!
//! # Limits
//!
//! Hard limits are enforced to prevent denial-of-service through oversized
//! inputs:
//!
//! | Resource      | Limit                  | Constant          |
//! |---------------|------------------------|-------------------|
//! | Nodes (rows)  | 10,000,000             | [`MAX_NODES`]     |
//! | Edges (nnz)   | 100,000,000            | [`MAX_EDGES`]     |
//! | Dimension     | 65,536                 | [`MAX_DIM`]       |
//! | Iterations    | 1,000,000              | [`MAX_ITERATIONS`]|
//! | Request body  | 10 MiB                 | [`MAX_BODY_SIZE`] |

use crate::error::ValidationError;
use crate::types::{CsrMatrix, SolverResult};

// ---------------------------------------------------------------------------
// Resource limits
// ---------------------------------------------------------------------------

/// Maximum number of rows or columns to prevent resource exhaustion.
pub const MAX_NODES: usize = 10_000_000;

/// Maximum number of non-zero entries.
pub const MAX_EDGES: usize = 100_000_000;

/// Maximum vector/matrix dimension for dense operations.
pub const MAX_DIM: usize = 65_536;

/// Maximum solver iterations to prevent runaway computation.
pub const MAX_ITERATIONS: usize = 1_000_000;

/// Maximum request body size in bytes (10 MiB).
pub const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

// ---------------------------------------------------------------------------
// CSR matrix validation
// ---------------------------------------------------------------------------

/// Validate the structural integrity of a CSR matrix.
///
/// Performs the following checks in order:
///
/// 1. `rows` and `cols` are within [`MAX_NODES`].
/// 2. `nnz` (number of non-zeros) is within [`MAX_EDGES`].
/// 3. `row_ptr` length equals `rows + 1`.
/// 4. `row_ptr` is monotonically non-decreasing.
/// 5. `row_ptr[0] == 0` and `row_ptr[rows] == nnz`.
/// 6. `col_indices` length equals `values` length.
/// 7. All column indices are less than `cols`.
/// 8. No `NaN` or `Inf` values in `values`.
/// 9. Column indices are sorted within each row (emits a [`tracing::warn`] if
///    not, but does not error).
///
/// # Errors
///
/// Returns [`ValidationError`] describing the first violation found.
///
/// # Examples
///
/// ```
/// use ruvector_solver::types::CsrMatrix;
/// use ruvector_solver::validation::validate_csr_matrix;
///
/// let m = CsrMatrix::<f32>::from_coo(2, 2, vec![(0, 0, 1.0), (1, 1, 2.0)]);
/// assert!(validate_csr_matrix(&m).is_ok());
/// ```
pub fn validate_csr_matrix(matrix: &CsrMatrix<f32>) -> Result<(), ValidationError> {
    // 1. Dimension bounds
    if matrix.rows > MAX_NODES || matrix.cols > MAX_NODES {
        return Err(ValidationError::MatrixTooLarge {
            rows: matrix.rows,
            cols: matrix.cols,
            max_dim: MAX_NODES,
        });
    }

    // 2. NNZ bounds
    let nnz = matrix.values.len();
    if nnz > MAX_EDGES {
        return Err(ValidationError::DimensionMismatch(format!(
            "nnz {} exceeds maximum allowed {}",
            nnz, MAX_EDGES,
        )));
    }

    // 3. row_ptr length
    let expected_row_ptr_len = matrix.rows + 1;
    if matrix.row_ptr.len() != expected_row_ptr_len {
        return Err(ValidationError::DimensionMismatch(format!(
            "row_ptr length {} does not equal rows + 1 = {}",
            matrix.row_ptr.len(),
            expected_row_ptr_len,
        )));
    }

    // 4. row_ptr monotonicity
    for i in 1..matrix.row_ptr.len() {
        if matrix.row_ptr[i] < matrix.row_ptr[i - 1] {
            return Err(ValidationError::NonMonotonicRowPtrs { position: i });
        }
    }

    // 5. row_ptr boundary values
    if matrix.row_ptr[0] != 0 {
        return Err(ValidationError::DimensionMismatch(format!(
            "row_ptr[0] = {} (expected 0)",
            matrix.row_ptr[0],
        )));
    }
    let expected_nnz = matrix.row_ptr[matrix.rows];
    if expected_nnz != nnz {
        return Err(ValidationError::DimensionMismatch(format!(
            "values length {} does not match row_ptr[rows] = {}",
            nnz, expected_nnz,
        )));
    }

    // 6. col_indices length must match values length
    if matrix.col_indices.len() != nnz {
        return Err(ValidationError::DimensionMismatch(format!(
            "col_indices length {} does not match values length {}",
            matrix.col_indices.len(),
            nnz,
        )));
    }

    // 7. Column index bounds + 9. Sorted check (warn only) + 8. Finiteness
    for row in 0..matrix.rows {
        let start = matrix.row_ptr[row];
        let end = matrix.row_ptr[row + 1];

        let mut prev_col: Option<usize> = None;
        for idx in start..end {
            let col = matrix.col_indices[idx];
            if col >= matrix.cols {
                return Err(ValidationError::IndexOutOfBounds {
                    index: col as u32,
                    row,
                    cols: matrix.cols,
                });
            }

            let val = matrix.values[idx];
            if !val.is_finite() {
                return Err(ValidationError::NonFiniteValue(format!(
                    "matrix[{}, {}] = {}",
                    row, col, val,
                )));
            }

            // Check sorted order within row (warn, not error)
            if let Some(pc) = prev_col {
                if col < pc {
                    tracing::warn!(
                        row = row,
                        "column indices not sorted within row (col {} follows {}); \
                         performance may be degraded",
                        col,
                        pc,
                    );
                }
            }
            prev_col = Some(col);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// RHS vector validation
// ---------------------------------------------------------------------------

/// Validate a right-hand-side vector for a linear solve.
///
/// Checks:
///
/// 1. `rhs.len() == expected_len` (dimension must match the matrix).
/// 2. No `NaN` or `Inf` entries.
/// 3. If all entries are zero, emits a [`tracing::warn`] (a zero RHS is
///    technically valid but often indicates a bug).
///
/// # Errors
///
/// Returns [`ValidationError`] on dimension mismatch or non-finite values.
pub fn validate_rhs(rhs: &[f32], expected_len: usize) -> Result<(), ValidationError> {
    // 1. Length check
    if rhs.len() != expected_len {
        return Err(ValidationError::DimensionMismatch(format!(
            "rhs length {} does not match expected {}",
            rhs.len(),
            expected_len,
        )));
    }

    // 2. Finite check + 3. All-zeros check
    let mut all_zero = true;
    for (i, &v) in rhs.iter().enumerate() {
        if !v.is_finite() {
            return Err(ValidationError::NonFiniteValue(format!(
                "rhs[{}] = {}",
                i, v,
            )));
        }
        if v != 0.0 {
            all_zero = false;
        }
    }

    if all_zero && !rhs.is_empty() {
        tracing::warn!("rhs vector is all zeros; solution will be trivially zero");
    }

    Ok(())
}

/// Validate the right-hand side vector `b` for compatibility with a matrix.
///
/// This is an alias for [`validate_rhs`] that preserves backward compatibility
/// with the original API name.
pub fn validate_rhs_vector(rhs: &[f32], expected_len: usize) -> Result<(), ValidationError> {
    validate_rhs(rhs, expected_len)
}

// ---------------------------------------------------------------------------
// Solver parameter validation
// ---------------------------------------------------------------------------

/// Validate solver convergence parameters.
///
/// # Rules
///
/// - `tolerance` must be in the range `(0.0, 1.0]` and be finite.
/// - `max_iterations` must be in `[1, MAX_ITERATIONS]`.
///
/// # Errors
///
/// Returns [`ValidationError::ParameterOutOfRange`] if either parameter is
/// outside its valid range.
pub fn validate_params(tolerance: f64, max_iterations: usize) -> Result<(), ValidationError> {
    if !tolerance.is_finite() || tolerance <= 0.0 || tolerance > 1.0 {
        return Err(ValidationError::ParameterOutOfRange {
            name: "tolerance".into(),
            value: format!("{tolerance:.2e}"),
            expected: "(0.0, 1.0]".into(),
        });
    }

    if max_iterations == 0 || max_iterations > MAX_ITERATIONS {
        return Err(ValidationError::ParameterOutOfRange {
            name: "max_iterations".into(),
            value: max_iterations.to_string(),
            expected: format!("[1, {}]", MAX_ITERATIONS),
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Combined solver input validation
// ---------------------------------------------------------------------------

/// Validate the complete solver input (matrix + rhs + parameters).
///
/// This is a convenience function that calls [`validate_csr_matrix`],
/// [`validate_rhs`], and validates tolerance in sequence. It also checks
/// that the matrix is square, which is required by all iterative solvers.
///
/// # Errors
///
/// Returns [`ValidationError`] on the first failing check.
pub fn validate_solver_input(
    matrix: &CsrMatrix<f32>,
    rhs: &[f32],
    tolerance: f64,
) -> Result<(), ValidationError> {
    validate_csr_matrix(matrix)?;
    validate_rhs(rhs, matrix.rows)?;

    // Square matrix required for iterative solvers.
    if matrix.rows != matrix.cols {
        return Err(ValidationError::DimensionMismatch(format!(
            "solver requires a square matrix but got {}x{}",
            matrix.rows, matrix.cols,
        )));
    }

    // Tolerance bounds.
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(ValidationError::ParameterOutOfRange {
            name: "tolerance".into(),
            value: tolerance.to_string(),
            expected: "finite positive value".into(),
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Output validation (post-solve)
// ---------------------------------------------------------------------------

/// Validate a solver result after computation completes.
///
/// This catches silent numerical corruption that may have occurred during
/// iteration:
///
/// 1. No `NaN` or `Inf` in the solution vector.
/// 2. The residual norm is finite.
/// 3. At least one iteration was performed.
///
/// # Errors
///
/// Returns [`ValidationError`] if the output is corrupted.
pub fn validate_output(result: &SolverResult) -> Result<(), ValidationError> {
    // 1. Solution vector finiteness
    for (i, &v) in result.solution.iter().enumerate() {
        if !v.is_finite() {
            return Err(ValidationError::NonFiniteValue(format!(
                "solution[{}] = {}",
                i, v,
            )));
        }
    }

    // 2. Residual finiteness
    if !result.residual_norm.is_finite() {
        return Err(ValidationError::NonFiniteValue(format!(
            "residual_norm = {}",
            result.residual_norm,
        )));
    }

    // 3. Iteration count
    if result.iterations == 0 {
        return Err(ValidationError::ParameterOutOfRange {
            name: "iterations".into(),
            value: "0".into(),
            expected: ">= 1".into(),
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Body size validation (for API / deserialization boundaries)
// ---------------------------------------------------------------------------

/// Validate that a request body does not exceed [`MAX_BODY_SIZE`].
///
/// Call this at the deserialization boundary before parsing untrusted input.
///
/// # Errors
///
/// Returns [`ValidationError::ParameterOutOfRange`] if `size > MAX_BODY_SIZE`.
pub fn validate_body_size(size: usize) -> Result<(), ValidationError> {
    if size > MAX_BODY_SIZE {
        return Err(ValidationError::ParameterOutOfRange {
            name: "body_size".into(),
            value: format!("{} bytes", size),
            expected: format!("<= {} bytes (10 MiB)", MAX_BODY_SIZE),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Algorithm, ConvergenceInfo, CsrMatrix, SolverResult};
    use std::time::Duration;

    fn make_identity(n: usize) -> CsrMatrix<f32> {
        let mut row_ptr = vec![0usize; n + 1];
        let mut col_indices = Vec::with_capacity(n);
        let mut values = Vec::with_capacity(n);
        for i in 0..n {
            row_ptr[i + 1] = i + 1;
            col_indices.push(i);
            values.push(1.0);
        }
        CsrMatrix {
            values,
            col_indices,
            row_ptr,
            rows: n,
            cols: n,
        }
    }

    // -- validate_csr_matrix ------------------------------------------------

    #[test]
    fn valid_identity() {
        let mat = make_identity(4);
        assert!(validate_csr_matrix(&mat).is_ok());
    }

    #[test]
    fn valid_empty_matrix() {
        let m = CsrMatrix {
            row_ptr: vec![0],
            col_indices: vec![],
            values: vec![],
            rows: 0,
            cols: 0,
        };
        assert!(validate_csr_matrix(&m).is_ok());
    }

    #[test]
    fn valid_from_coo() {
        let m = CsrMatrix::<f32>::from_coo(
            3,
            3,
            vec![
                (0, 0, 2.0),
                (0, 1, -0.5),
                (1, 0, -0.5),
                (1, 1, 2.0),
                (1, 2, -0.5),
                (2, 1, -0.5),
                (2, 2, 2.0),
            ],
        );
        assert!(validate_csr_matrix(&m).is_ok());
    }

    #[test]
    fn rejects_too_large_matrix() {
        let m = CsrMatrix {
            row_ptr: vec![0, 0],
            col_indices: vec![],
            values: vec![],
            rows: MAX_NODES + 1,
            cols: 1,
        };
        assert!(matches!(
            validate_csr_matrix(&m),
            Err(ValidationError::MatrixTooLarge { .. })
        ));
    }

    #[test]
    fn rejects_wrong_row_ptr_length() {
        let m = CsrMatrix {
            row_ptr: vec![0, 1],
            col_indices: vec![0],
            values: vec![1.0],
            rows: 3,
            cols: 3,
        };
        assert!(matches!(
            validate_csr_matrix(&m),
            Err(ValidationError::DimensionMismatch(_))
        ));
    }

    #[test]
    fn non_monotonic_row_ptr() {
        let mut mat = make_identity(4);
        mat.row_ptr[2] = 0; // break monotonicity
        let err = validate_csr_matrix(&mat).unwrap_err();
        assert!(matches!(err, ValidationError::NonMonotonicRowPtrs { .. }));
    }

    #[test]
    fn rejects_row_ptr_not_starting_at_zero() {
        let m = CsrMatrix {
            row_ptr: vec![1, 2],
            col_indices: vec![0],
            values: vec![1.0],
            rows: 1,
            cols: 1,
        };
        match validate_csr_matrix(&m) {
            Err(ValidationError::DimensionMismatch(msg)) => {
                assert!(msg.contains("row_ptr[0]"), "msg: {msg}");
            }
            other => panic!("expected DimensionMismatch for row_ptr[0], got {other:?}"),
        }
    }

    #[test]
    fn col_index_out_of_bounds() {
        let mut mat = make_identity(4);
        mat.col_indices[1] = 99;
        let err = validate_csr_matrix(&mat).unwrap_err();
        assert!(matches!(err, ValidationError::IndexOutOfBounds { .. }));
    }

    #[test]
    fn nan_value_rejected() {
        let mut mat = make_identity(4);
        mat.values[0] = f32::NAN;
        let err = validate_csr_matrix(&mat).unwrap_err();
        assert!(matches!(err, ValidationError::NonFiniteValue(_)));
    }

    #[test]
    fn inf_value_rejected() {
        let mut mat = make_identity(4);
        mat.values[0] = f32::INFINITY;
        let err = validate_csr_matrix(&mat).unwrap_err();
        assert!(matches!(err, ValidationError::NonFiniteValue(_)));
    }

    // -- validate_rhs -------------------------------------------------------

    #[test]
    fn valid_rhs() {
        assert!(validate_rhs(&[1.0, 2.0, 3.0], 3).is_ok());
    }

    #[test]
    fn rhs_dimension_mismatch() {
        let err = validate_rhs(&[1.0, 2.0], 3).unwrap_err();
        assert!(matches!(err, ValidationError::DimensionMismatch(_)));
    }

    #[test]
    fn rhs_nan_rejected() {
        let err = validate_rhs(&[1.0, f32::NAN, 3.0], 3).unwrap_err();
        assert!(matches!(err, ValidationError::NonFiniteValue(_)));
    }

    #[test]
    fn rhs_inf_rejected() {
        let err = validate_rhs(&[1.0, f32::NEG_INFINITY, 3.0], 3).unwrap_err();
        assert!(matches!(err, ValidationError::NonFiniteValue(_)));
    }

    #[test]
    fn warns_on_all_zero_rhs() {
        // Should succeed but emit a warning (cannot assert warning in unit test,
        // but at least verify it does not error).
        assert!(validate_rhs(&[0.0, 0.0, 0.0], 3).is_ok());
    }

    // -- validate_rhs_vector (backward compat alias) ------------------------

    #[test]
    fn rhs_vector_alias_works() {
        assert!(validate_rhs_vector(&[1.0, 2.0], 2).is_ok());
        assert!(validate_rhs_vector(&[1.0, 2.0], 3).is_err());
    }

    // -- validate_params ----------------------------------------------------

    #[test]
    fn valid_params() {
        assert!(validate_params(1e-8, 500).is_ok());
        assert!(validate_params(1.0, 1).is_ok());
    }

    #[test]
    fn rejects_zero_tolerance() {
        match validate_params(0.0, 100) {
            Err(ValidationError::ParameterOutOfRange { ref name, .. }) => {
                assert_eq!(name, "tolerance");
            }
            other => panic!("expected ParameterOutOfRange for tolerance, got {other:?}"),
        }
    }

    #[test]
    fn rejects_negative_tolerance() {
        match validate_params(-1e-6, 100) {
            Err(ValidationError::ParameterOutOfRange { ref name, .. }) => {
                assert_eq!(name, "tolerance");
            }
            other => panic!("expected ParameterOutOfRange for tolerance, got {other:?}"),
        }
    }

    #[test]
    fn rejects_tolerance_above_one() {
        match validate_params(1.5, 100) {
            Err(ValidationError::ParameterOutOfRange { ref name, .. }) => {
                assert_eq!(name, "tolerance");
            }
            other => panic!("expected ParameterOutOfRange for tolerance, got {other:?}"),
        }
    }

    #[test]
    fn rejects_nan_tolerance() {
        match validate_params(f64::NAN, 100) {
            Err(ValidationError::ParameterOutOfRange { ref name, .. }) => {
                assert_eq!(name, "tolerance");
            }
            other => panic!("expected ParameterOutOfRange for tolerance, got {other:?}"),
        }
    }

    #[test]
    fn rejects_zero_iterations() {
        match validate_params(1e-6, 0) {
            Err(ValidationError::ParameterOutOfRange { ref name, .. }) => {
                assert_eq!(name, "max_iterations");
            }
            other => panic!("expected ParameterOutOfRange for max_iterations, got {other:?}"),
        }
    }

    #[test]
    fn rejects_excessive_iterations() {
        match validate_params(1e-6, MAX_ITERATIONS + 1) {
            Err(ValidationError::ParameterOutOfRange { ref name, .. }) => {
                assert_eq!(name, "max_iterations");
            }
            other => panic!("expected ParameterOutOfRange for max_iterations, got {other:?}"),
        }
    }

    // -- validate_solver_input (combined) -----------------------------------

    #[test]
    fn full_input_validation() {
        let mat = make_identity(3);
        let rhs = vec![1.0f32, 2.0, 3.0];
        assert!(validate_solver_input(&mat, &rhs, 1e-6).is_ok());
    }

    #[test]
    fn non_square_rejected() {
        let mat = CsrMatrix {
            values: vec![],
            col_indices: vec![],
            row_ptr: vec![0, 0, 0],
            rows: 2,
            cols: 3,
        };
        let rhs = vec![1.0f32, 2.0];
        let err = validate_solver_input(&mat, &rhs, 1e-6).unwrap_err();
        assert!(matches!(err, ValidationError::DimensionMismatch(_)));
    }

    #[test]
    fn invalid_tolerance_rejected() {
        let mat = make_identity(2);
        let rhs = vec![1.0f32, 2.0];
        assert!(validate_solver_input(&mat, &rhs, -1.0).is_err());
        assert!(validate_solver_input(&mat, &rhs, 0.0).is_err());
        assert!(validate_solver_input(&mat, &rhs, f64::NAN).is_err());
    }

    // -- validate_output ----------------------------------------------------

    #[test]
    fn valid_output() {
        let result = SolverResult {
            solution: vec![1.0, 2.0, 3.0],
            iterations: 10,
            residual_norm: 1e-8,
            wall_time: Duration::from_millis(5),
            convergence_history: vec![ConvergenceInfo {
                iteration: 0,
                residual_norm: 1.0,
            }],
            algorithm: Algorithm::Neumann,
        };
        assert!(validate_output(&result).is_ok());
    }

    #[test]
    fn rejects_nan_in_solution() {
        let result = SolverResult {
            solution: vec![1.0, f32::NAN, 3.0],
            iterations: 1,
            residual_norm: 1e-8,
            wall_time: Duration::from_millis(1),
            convergence_history: vec![],
            algorithm: Algorithm::Neumann,
        };
        match validate_output(&result) {
            Err(ValidationError::NonFiniteValue(ref msg)) => {
                assert!(msg.contains("solution"), "msg: {msg}");
            }
            other => panic!("expected NonFiniteValue for solution, got {other:?}"),
        }
    }

    #[test]
    fn rejects_inf_in_solution() {
        let result = SolverResult {
            solution: vec![f32::INFINITY],
            iterations: 1,
            residual_norm: 1e-8,
            wall_time: Duration::from_millis(1),
            convergence_history: vec![],
            algorithm: Algorithm::Neumann,
        };
        match validate_output(&result) {
            Err(ValidationError::NonFiniteValue(ref msg)) => {
                assert!(msg.contains("solution"), "msg: {msg}");
            }
            other => panic!("expected NonFiniteValue for solution, got {other:?}"),
        }
    }

    #[test]
    fn rejects_nan_residual() {
        let result = SolverResult {
            solution: vec![1.0],
            iterations: 1,
            residual_norm: f64::NAN,
            wall_time: Duration::from_millis(1),
            convergence_history: vec![],
            algorithm: Algorithm::Neumann,
        };
        match validate_output(&result) {
            Err(ValidationError::NonFiniteValue(ref msg)) => {
                assert!(msg.contains("residual"), "msg: {msg}");
            }
            other => panic!("expected NonFiniteValue for residual, got {other:?}"),
        }
    }

    #[test]
    fn rejects_inf_residual() {
        let result = SolverResult {
            solution: vec![1.0],
            iterations: 1,
            residual_norm: f64::INFINITY,
            wall_time: Duration::from_millis(1),
            convergence_history: vec![],
            algorithm: Algorithm::Neumann,
        };
        assert!(matches!(
            validate_output(&result),
            Err(ValidationError::NonFiniteValue(_))
        ));
    }

    #[test]
    fn rejects_zero_iterations_in_output() {
        let result = SolverResult {
            solution: vec![1.0],
            iterations: 0,
            residual_norm: 1e-8,
            wall_time: Duration::from_millis(1),
            convergence_history: vec![],
            algorithm: Algorithm::Neumann,
        };
        match validate_output(&result) {
            Err(ValidationError::ParameterOutOfRange { ref name, .. }) => {
                assert_eq!(name, "iterations");
            }
            other => panic!("expected ParameterOutOfRange, got {other:?}"),
        }
    }

    // -- validate_body_size -------------------------------------------------

    #[test]
    fn valid_body_size() {
        assert!(validate_body_size(1024).is_ok());
        assert!(validate_body_size(MAX_BODY_SIZE).is_ok());
    }

    #[test]
    fn rejects_oversized_body() {
        match validate_body_size(MAX_BODY_SIZE + 1) {
            Err(ValidationError::ParameterOutOfRange { ref name, .. }) => {
                assert_eq!(name, "body_size");
            }
            other => panic!("expected ParameterOutOfRange, got {other:?}"),
        }
    }
}
