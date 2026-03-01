//! Integration tests for input validation.
//!
//! Tests cover rejection of NaN and Inf values, dimension mismatches,
//! malformed CSR structures (non-monotonic row_ptrs), and oversized inputs.

use ruvector_solver::error::ValidationError;
use ruvector_solver::types::CsrMatrix;
use ruvector_solver::validation::{
    validate_csr_matrix, validate_rhs, validate_solver_input, MAX_NODES,
};

// ---------------------------------------------------------------------------
// Helper: build a valid f32 identity matrix
// ---------------------------------------------------------------------------

fn identity_f32(n: usize) -> CsrMatrix<f32> {
    let row_ptr: Vec<usize> = (0..=n).collect();
    let col_indices: Vec<usize> = (0..n).collect();
    let values = vec![1.0f32; n];
    CsrMatrix {
        row_ptr,
        col_indices,
        values,
        rows: n,
        cols: n,
    }
}

/// Build a small valid 3x3 f32 CSR matrix for testing.
fn valid_3x3_f32() -> CsrMatrix<f32> {
    CsrMatrix::<f32>::from_coo(
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
    )
}

// ---------------------------------------------------------------------------
// Reject NaN in matrix values
// ---------------------------------------------------------------------------

#[test]
fn test_reject_nan_input() {
    // NaN in matrix values should be rejected.
    let mut mat = identity_f32(4);
    mat.values[2] = f32::NAN;

    let err = validate_csr_matrix(&mat).unwrap_err();
    assert!(
        matches!(err, ValidationError::NonFiniteValue(_)),
        "expected NonFiniteValue for NaN in matrix, got {:?}",
        err
    );

    // NaN in RHS should be rejected.
    let rhs = vec![1.0f32, f32::NAN, 3.0];
    let err_rhs = validate_rhs(&rhs, 3).unwrap_err();
    assert!(
        matches!(err_rhs, ValidationError::NonFiniteValue(_)),
        "expected NonFiniteValue for NaN in RHS, got {:?}",
        err_rhs
    );

    // Combined validation should also catch NaN in matrix.
    let mut mat2 = valid_3x3_f32();
    mat2.values[0] = f32::NAN;
    let err_combined = validate_solver_input(&mat2, &[1.0, 2.0, 3.0], 1e-6).unwrap_err();
    assert!(
        matches!(err_combined, ValidationError::NonFiniteValue(_)),
        "combined validation should reject NaN matrix values"
    );
}

// ---------------------------------------------------------------------------
// Reject Inf values
// ---------------------------------------------------------------------------

#[test]
fn test_reject_inf_input() {
    // Positive infinity in matrix values.
    let mut mat = identity_f32(3);
    mat.values[0] = f32::INFINITY;

    let err = validate_csr_matrix(&mat).unwrap_err();
    assert!(
        matches!(err, ValidationError::NonFiniteValue(_)),
        "expected NonFiniteValue for +Inf in matrix, got {:?}",
        err
    );

    // Negative infinity in matrix values.
    let mut mat2 = identity_f32(3);
    mat2.values[1] = f32::NEG_INFINITY;

    let err2 = validate_csr_matrix(&mat2).unwrap_err();
    assert!(
        matches!(err2, ValidationError::NonFiniteValue(_)),
        "expected NonFiniteValue for -Inf in matrix, got {:?}",
        err2
    );

    // Infinity in RHS.
    let rhs = vec![1.0f32, f32::INFINITY, 3.0];
    let err_rhs = validate_rhs(&rhs, 3).unwrap_err();
    assert!(
        matches!(err_rhs, ValidationError::NonFiniteValue(_)),
        "expected NonFiniteValue for Inf in RHS, got {:?}",
        err_rhs
    );

    // Negative infinity in RHS.
    let rhs_neg = vec![f32::NEG_INFINITY, 2.0, 3.0];
    let err_neg = validate_rhs(&rhs_neg, 3).unwrap_err();
    assert!(
        matches!(err_neg, ValidationError::NonFiniteValue(_)),
        "expected NonFiniteValue for -Inf in RHS, got {:?}",
        err_neg
    );
}

// ---------------------------------------------------------------------------
// Reject dimension mismatch: rhs length != matrix rows
// ---------------------------------------------------------------------------

#[test]
fn test_reject_dimension_mismatch() {
    // RHS too short.
    let err_short = validate_rhs(&[1.0f32, 2.0], 3).unwrap_err();
    assert!(
        matches!(err_short, ValidationError::DimensionMismatch(_)),
        "expected DimensionMismatch for rhs length < expected, got {:?}",
        err_short
    );

    // RHS too long.
    let err_long = validate_rhs(&[1.0f32, 2.0, 3.0, 4.0], 3).unwrap_err();
    assert!(
        matches!(err_long, ValidationError::DimensionMismatch(_)),
        "expected DimensionMismatch for rhs length > expected, got {:?}",
        err_long
    );

    // Combined solver input validation: rhs doesn't match matrix rows.
    let mat = valid_3x3_f32();
    let rhs_wrong = vec![1.0f32, 2.0]; // length 2, but matrix is 3x3
    let err_combined = validate_solver_input(&mat, &rhs_wrong, 1e-6).unwrap_err();
    assert!(
        matches!(err_combined, ValidationError::DimensionMismatch(_)),
        "combined validation should reject dimension mismatch, got {:?}",
        err_combined
    );

    // Non-square matrix should be rejected by validate_solver_input.
    let non_square = CsrMatrix::<f32> {
        row_ptr: vec![0, 0, 0],
        col_indices: vec![],
        values: vec![],
        rows: 2,
        cols: 3,
    };
    let rhs_ns = vec![1.0f32, 2.0];
    let err_ns = validate_solver_input(&non_square, &rhs_ns, 1e-6).unwrap_err();
    assert!(
        matches!(err_ns, ValidationError::DimensionMismatch(_)),
        "non-square matrix should be rejected, got {:?}",
        err_ns
    );
}

// ---------------------------------------------------------------------------
// Reject invalid CSR: row_ptrs not monotonic
// ---------------------------------------------------------------------------

#[test]
fn test_reject_invalid_csr() {
    // Break monotonicity of row_ptr.
    let mut mat = identity_f32(4);
    // row_ptr is [0, 1, 2, 3, 4]. Set row_ptr[2] = 0 to break monotonicity.
    mat.row_ptr[2] = 0;

    let err = validate_csr_matrix(&mat).unwrap_err();
    assert!(
        matches!(err, ValidationError::NonMonotonicRowPtrs { .. }),
        "expected NonMonotonicRowPtrs, got {:?}",
        err
    );

    // Verify the position is reported.
    if let ValidationError::NonMonotonicRowPtrs { position } = err {
        assert_eq!(
            position, 2,
            "monotonicity violation should be at position 2"
        );
    }

    // row_ptr[0] != 0 should also be rejected.
    let bad_start = CsrMatrix::<f32> {
        row_ptr: vec![1, 2],
        col_indices: vec![0],
        values: vec![1.0],
        rows: 1,
        cols: 1,
    };
    let err_start = validate_csr_matrix(&bad_start).unwrap_err();
    assert!(
        matches!(err_start, ValidationError::DimensionMismatch(_)),
        "row_ptr[0] != 0 should be rejected, got {:?}",
        err_start
    );

    // row_ptr length != rows + 1 should be rejected.
    let bad_len = CsrMatrix::<f32> {
        row_ptr: vec![0, 1], // length 2, but rows = 3 so expect length 4
        col_indices: vec![0],
        values: vec![1.0],
        rows: 3,
        cols: 3,
    };
    let err_len = validate_csr_matrix(&bad_len).unwrap_err();
    assert!(
        matches!(err_len, ValidationError::DimensionMismatch(_)),
        "wrong row_ptr length should be rejected, got {:?}",
        err_len
    );

    // Column index out of bounds.
    let mut bad_col = identity_f32(3);
    bad_col.col_indices[1] = 99; // out of bounds
    let err_col = validate_csr_matrix(&bad_col).unwrap_err();
    assert!(
        matches!(err_col, ValidationError::IndexOutOfBounds { .. }),
        "column index out of bounds should be rejected, got {:?}",
        err_col
    );
}

// ---------------------------------------------------------------------------
// Reject oversized input: exceeds MAX_NODES
// ---------------------------------------------------------------------------

#[test]
fn test_reject_oversized_input() {
    // Matrix with rows > MAX_NODES.
    let oversized = CsrMatrix::<f32> {
        row_ptr: vec![0; MAX_NODES + 2], // length = (MAX_NODES + 1) + 1
        col_indices: vec![],
        values: vec![],
        rows: MAX_NODES + 1,
        cols: 1,
    };

    let err = validate_csr_matrix(&oversized).unwrap_err();
    assert!(
        matches!(err, ValidationError::MatrixTooLarge { .. }),
        "expected MatrixTooLarge for rows > MAX_NODES, got {:?}",
        err
    );

    // Verify the reported dimensions.
    if let ValidationError::MatrixTooLarge {
        rows,
        cols,
        max_dim,
    } = err
    {
        assert_eq!(rows, MAX_NODES + 1);
        assert_eq!(cols, 1);
        assert_eq!(max_dim, MAX_NODES);
    }

    // Matrix with cols > MAX_NODES.
    let oversized_cols = CsrMatrix::<f32> {
        row_ptr: vec![0, 0],
        col_indices: vec![],
        values: vec![],
        rows: 1,
        cols: MAX_NODES + 1,
    };

    let err_cols = validate_csr_matrix(&oversized_cols).unwrap_err();
    assert!(
        matches!(err_cols, ValidationError::MatrixTooLarge { .. }),
        "expected MatrixTooLarge for cols > MAX_NODES, got {:?}",
        err_cols
    );

    // A matrix at exactly MAX_NODES should be accepted (boundary check).
    // We cannot allocate MAX_NODES entries in a test, but verify the logic:
    // MAX_NODES rows with 0 nnz should be valid structurally.
    // (Skipping actual allocation of 10M-entry row_ptr for test speed.)
}
