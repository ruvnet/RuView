//! Integration tests for `CsrMatrix` — construction, SpMV, transpose, and
//! structural queries.

mod helpers;

use approx::assert_relative_eq;
use ruvector_solver::types::CsrMatrix;

// ---------------------------------------------------------------------------
// Construction from COO triplets
// ---------------------------------------------------------------------------

#[test]
fn test_csr_from_triplets() {
    // 3x3 matrix:
    //  [ 2  -1   0 ]
    //  [-1   3  -1 ]
    //  [ 0  -1   2 ]
    let triplets: Vec<(usize, usize, f64)> = vec![
        (0, 0, 2.0),
        (0, 1, -1.0),
        (1, 0, -1.0),
        (1, 1, 3.0),
        (1, 2, -1.0),
        (2, 1, -1.0),
        (2, 2, 2.0),
    ];

    let mat = CsrMatrix::<f64>::from_coo(3, 3, triplets);

    assert_eq!(mat.rows, 3);
    assert_eq!(mat.cols, 3);
    assert_eq!(mat.values.len(), 7);
    assert_eq!(mat.col_indices.len(), 7);
    assert_eq!(mat.row_ptr.len(), 4); // rows + 1

    // Verify row_ptr encodes correct row boundaries.
    assert_eq!(mat.row_ptr[0], 0);
    assert_eq!(mat.row_ptr[1], 2); // row 0 has 2 entries
    assert_eq!(mat.row_ptr[2], 5); // row 1 has 3 entries
    assert_eq!(mat.row_ptr[3], 7); // row 2 has 2 entries

    // Verify row_degree for each row.
    assert_eq!(mat.row_degree(0), 2);
    assert_eq!(mat.row_degree(1), 3);
    assert_eq!(mat.row_degree(2), 2);
}

// ---------------------------------------------------------------------------
// SpMV correctness vs dense multiply
// ---------------------------------------------------------------------------

#[test]
fn test_csr_spmv() {
    // A = [ 2  -1   0 ]    x = [1]    Ax = [ 2 - 1 + 0 ] = [ 1 ]
    //     [-1   3  -1 ]        [1]         [-1 + 3 - 1 ] = [ 1 ]
    //     [ 0  -1   2 ]        [1]         [ 0 - 1 + 2 ] = [ 1 ]
    let mat = CsrMatrix::<f64>::from_coo(
        3,
        3,
        vec![
            (0, 0, 2.0),
            (0, 1, -1.0),
            (1, 0, -1.0),
            (1, 1, 3.0),
            (1, 2, -1.0),
            (2, 1, -1.0),
            (2, 2, 2.0),
        ],
    );

    let x = vec![1.0, 1.0, 1.0];
    let mut y = vec![0.0f64; 3];
    mat.spmv(&x, &mut y);

    assert_relative_eq!(y[0], 1.0, epsilon = 1e-12);
    assert_relative_eq!(y[1], 1.0, epsilon = 1e-12);
    assert_relative_eq!(y[2], 1.0, epsilon = 1e-12);

    // Non-trivial x.
    let x2 = vec![1.0, 2.0, 3.0];
    let mut y2 = vec![0.0f64; 3];
    mat.spmv(&x2, &mut y2);

    // Manual: A*[1,2,3] = [2*1 + (-1)*2, -1*1 + 3*2 + (-1)*3, (-1)*2 + 2*3]
    //                    = [0, 2, 4]
    assert_relative_eq!(y2[0], 0.0, epsilon = 1e-12);
    assert_relative_eq!(y2[1], 2.0, epsilon = 1e-12);
    assert_relative_eq!(y2[2], 4.0, epsilon = 1e-12);
}

// ---------------------------------------------------------------------------
// Density calculation
// ---------------------------------------------------------------------------

#[test]
fn test_csr_density() {
    // 4x4 matrix with 6 non-zero entries
    let mat = CsrMatrix::<f64>::from_coo(
        4,
        4,
        vec![
            (0, 0, 1.0),
            (1, 1, 2.0),
            (2, 2, 3.0),
            (3, 3, 4.0),
            (0, 1, 0.5),
            (1, 0, 0.5),
        ],
    );

    let nnz = mat.values.len();
    let total_entries = mat.rows * mat.cols;
    let density = nnz as f64 / total_entries as f64;

    assert_eq!(nnz, 6);
    assert_relative_eq!(density, 6.0 / 16.0, epsilon = 1e-12);
}

// ---------------------------------------------------------------------------
// Transpose correctness
// ---------------------------------------------------------------------------

#[test]
fn test_csr_transpose() {
    // Non-symmetric matrix:
    //  A = [ 1  2  0 ]      A^T = [ 1  0  3 ]
    //      [ 0  3  0 ]            [ 2  3  0 ]
    //      [ 3  0  4 ]            [ 0  0  4 ]
    let mat = CsrMatrix::<f64>::from_coo(
        3,
        3,
        vec![
            (0, 0, 1.0),
            (0, 1, 2.0),
            (1, 1, 3.0),
            (2, 0, 3.0),
            (2, 2, 4.0),
        ],
    );

    let at = mat.transpose();

    assert_eq!(at.rows, 3);
    assert_eq!(at.cols, 3);
    assert_eq!(at.values.len(), 5);

    // Verify A^T * e_0. Since A^T[i][j] = A[j][i], the first column of A^T
    // is the first row of A: [1, 2, 0].
    let e0 = vec![1.0, 0.0, 0.0];
    let mut y = vec![0.0f64; 3];
    at.spmv(&e0, &mut y);
    // (A^T * e_0)[i] = A^T[i][0] = A[0][i], so [A[0][0], A[0][1], A[0][2]] = [1, 2, 0]
    assert_relative_eq!(y[0], 1.0, epsilon = 1e-12);
    assert_relative_eq!(y[1], 2.0, epsilon = 1e-12);
    assert_relative_eq!(y[2], 0.0, epsilon = 1e-12);

    // Verify transpose of transpose recovers the original.
    let att = at.transpose();
    let x = vec![1.0, 2.0, 3.0];
    let mut y_orig = vec![0.0f64; 3];
    let mut y_double = vec![0.0f64; 3];
    mat.spmv(&x, &mut y_orig);
    att.spmv(&x, &mut y_double);
    for i in 0..3 {
        assert_relative_eq!(y_orig[i], y_double[i], epsilon = 1e-12);
    }
}

// ---------------------------------------------------------------------------
// Empty matrix handling
// ---------------------------------------------------------------------------

#[test]
fn test_csr_empty() {
    let mat = CsrMatrix::<f64>::from_coo(0, 0, Vec::<(usize, usize, f64)>::new());

    assert_eq!(mat.rows, 0);
    assert_eq!(mat.cols, 0);
    assert_eq!(mat.values.len(), 0);
    assert_eq!(mat.row_ptr.len(), 1); // [0]
    assert_eq!(mat.row_ptr[0], 0);

    // SpMV on empty should work trivially (no-op).
    let x: Vec<f64> = vec![];
    let mut y: Vec<f64> = vec![];
    mat.spmv(&x, &mut y);

    // Transpose of empty is empty.
    let at = mat.transpose();
    assert_eq!(at.rows, 0);
    assert_eq!(at.cols, 0);

    // Matrix with rows but no entries.
    let mat2 = CsrMatrix::<f64>::from_coo(3, 3, Vec::<(usize, usize, f64)>::new());
    assert_eq!(mat2.values.len(), 0);
    let mut y2 = vec![0.0f64; 3];
    mat2.spmv(&[1.0, 2.0, 3.0], &mut y2);
    for &v in &y2 {
        assert_relative_eq!(v, 0.0, epsilon = 1e-15);
    }
}

// ---------------------------------------------------------------------------
// Identity matrix: SpMV(I, x) = x
// ---------------------------------------------------------------------------

#[test]
fn test_csr_identity() {
    for n in [1, 5, 20, 100] {
        let identity = CsrMatrix::<f64>::identity(n);

        assert_eq!(identity.rows, n);
        assert_eq!(identity.cols, n);
        assert_eq!(identity.values.len(), n);

        // Generate a deterministic test vector.
        let x: Vec<f64> = (0..n).map(|i| (i as f64 + 1.0) * 0.7).collect();
        let mut y = vec![0.0f64; n];
        identity.spmv(&x, &mut y);

        for i in 0..n {
            assert_relative_eq!(y[i], x[i], epsilon = 1e-12);
        }
    }
}

// ---------------------------------------------------------------------------
// Diagonal matrix correctness
// ---------------------------------------------------------------------------

#[test]
fn test_csr_diagonal() {
    let diag_vals = vec![2.0, 3.0, 5.0, 7.0];
    let n = diag_vals.len();

    let entries: Vec<(usize, usize, f64)> = diag_vals
        .iter()
        .enumerate()
        .map(|(i, &v)| (i, i, v))
        .collect();
    let mat = CsrMatrix::<f64>::from_coo(n, n, entries);

    // D * x = element-wise product of diag and x.
    let x = vec![1.0, 2.0, 3.0, 4.0];
    let mut y = vec![0.0f64; n];
    mat.spmv(&x, &mut y);

    for i in 0..n {
        assert_relative_eq!(y[i], diag_vals[i] * x[i], epsilon = 1e-12);
    }

    // Verify each row has exactly 1 non-zero.
    for i in 0..n {
        assert_eq!(mat.row_degree(i), 1);
    }

    // Transpose of diagonal is the same matrix.
    let dt = mat.transpose();
    let mut yt = vec![0.0f64; n];
    dt.spmv(&x, &mut yt);
    for i in 0..n {
        assert_relative_eq!(y[i], yt[i], epsilon = 1e-12);
    }
}

// ---------------------------------------------------------------------------
// Symmetric detection (structural)
// ---------------------------------------------------------------------------

#[test]
fn test_csr_symmetric() {
    // Symmetric matrix.
    let sym = CsrMatrix::<f64>::from_coo(
        3,
        3,
        vec![
            (0, 0, 2.0),
            (0, 1, 1.0),
            (1, 0, 1.0),
            (1, 1, 3.0),
            (1, 2, 1.0),
            (2, 1, 1.0),
            (2, 2, 2.0),
        ],
    );

    // Check symmetry by comparing A and A^T via SpMV.
    let at = sym.transpose();
    let x = vec![1.0, 2.0, 3.0];
    let mut y_a = vec![0.0f64; 3];
    let mut y_at = vec![0.0f64; 3];
    sym.spmv(&x, &mut y_a);
    at.spmv(&x, &mut y_at);

    for i in 0..3 {
        assert_relative_eq!(y_a[i], y_at[i], epsilon = 1e-12);
    }

    // Use the orchestrator's sparsity analysis to check symmetry detection.
    let profile = ruvector_solver::router::SolverOrchestrator::analyze_sparsity(&sym);
    assert!(
        profile.is_symmetric_structure,
        "symmetric matrix should be detected as symmetric"
    );

    // Non-symmetric matrix.
    let asym = CsrMatrix::<f64>::from_coo(
        3,
        3,
        vec![
            (0, 0, 1.0),
            (0, 1, 2.0),
            // no (1, 0) entry
            (1, 1, 3.0),
            (2, 2, 4.0),
        ],
    );

    let profile_asym = ruvector_solver::router::SolverOrchestrator::analyze_sparsity(&asym);
    assert!(
        !profile_asym.is_symmetric_structure,
        "asymmetric matrix should not be detected as symmetric"
    );
}
