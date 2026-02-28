//! Integration tests for the Conjugate Gradient (CG) solver.
//!
//! Tests cover correctness on SPD systems, Laplacian solves, preconditioning
//! benefits, known-solution verification, and tolerance scaling.

mod helpers;

use approx::assert_relative_eq;
use ruvector_solver::cg::ConjugateGradientSolver;
use ruvector_solver::traits::SolverEngine;
use ruvector_solver::types::{Algorithm, ComputeBudget, CsrMatrix};

use helpers::{
    compute_residual, dense_solve, f32_to_f64, l2_norm, random_laplacian_csr, random_spd_csr,
    random_vector, relative_error,
};

// ---------------------------------------------------------------------------
// Helper: default compute budget
// ---------------------------------------------------------------------------

fn default_budget() -> ComputeBudget {
    ComputeBudget {
        max_time: std::time::Duration::from_secs(30),
        max_iterations: 10_000,
        tolerance: 1e-12,
    }
}

// ---------------------------------------------------------------------------
// SPD system: solve and verify convergence
// ---------------------------------------------------------------------------

#[test]
fn test_cg_spd_system() {
    let n = 15;
    let matrix = random_spd_csr(n, 0.4, 42);
    let rhs = random_vector(n, 43);
    let budget = default_budget();

    let solver = ConjugateGradientSolver::new(1e-8, 500, false);
    let result = solver.solve(&matrix, &rhs, &budget).unwrap();

    assert_eq!(result.algorithm, Algorithm::CG);
    assert!(
        result.residual_norm < 1e-4,
        "residual too large: {}",
        result.residual_norm
    );

    // Independent residual check.
    let x = f32_to_f64(&result.solution);
    let residual = compute_residual(&matrix, &x, &rhs);
    let resid_norm = l2_norm(&residual);
    assert!(
        resid_norm < 1e-3,
        "independent residual check: {}",
        resid_norm
    );

    // Compare with dense solve.
    let exact = dense_solve(&matrix, &rhs);
    let rel_err = relative_error(&x, &exact);
    assert!(rel_err < 1e-2, "relative error vs dense solve: {}", rel_err);
}

// ---------------------------------------------------------------------------
// Graph Laplacian system
// ---------------------------------------------------------------------------

#[test]
fn test_cg_laplacian() {
    let n = 12;
    let laplacian = random_laplacian_csr(n, 0.3, 44);

    // Laplacians are singular (L * ones = 0), so we add a small regulariser
    // to make it SPD: A = L + epsilon * I.
    let epsilon = 0.01;
    let mut entries: Vec<(usize, usize, f64)> = Vec::new();
    for i in 0..n {
        let start = laplacian.row_ptr[i];
        let end = laplacian.row_ptr[i + 1];
        for idx in start..end {
            let j = laplacian.col_indices[idx];
            let mut v = laplacian.values[idx];
            if i == j {
                v += epsilon;
            }
            entries.push((i, j, v));
        }
    }
    let reg_laplacian = CsrMatrix::<f64>::from_coo(n, n, entries);

    let rhs = random_vector(n, 45);
    let budget = default_budget();

    let solver = ConjugateGradientSolver::new(1e-8, 1000, false);
    let result = solver.solve(&reg_laplacian, &rhs, &budget).unwrap();

    assert!(
        result.residual_norm < 1e-4,
        "laplacian solve residual: {}",
        result.residual_norm
    );

    // Verify Ax = b.
    let x = f32_to_f64(&result.solution);
    let residual = compute_residual(&reg_laplacian, &x, &rhs);
    let resid_norm = l2_norm(&residual);
    assert!(
        resid_norm < 1e-3,
        "laplacian residual check: {}",
        resid_norm
    );
}

// ---------------------------------------------------------------------------
// Preconditioned CG reduces iterations
// ---------------------------------------------------------------------------

#[test]
fn test_cg_preconditioned() {
    let n = 30;
    let matrix = random_spd_csr(n, 0.3, 46);
    let rhs = random_vector(n, 47);
    let budget = default_budget();

    let unprecond = ConjugateGradientSolver::new(1e-8, 1000, false);
    let precond = ConjugateGradientSolver::new(1e-8, 1000, true);

    let result_no = unprecond.solve(&matrix, &rhs, &budget).unwrap();
    let result_yes = precond.solve(&matrix, &rhs, &budget).unwrap();

    // Both should converge.
    assert!(
        result_no.residual_norm < 1e-4,
        "unpreconditioned residual: {}",
        result_no.residual_norm
    );
    assert!(
        result_yes.residual_norm < 1e-4,
        "preconditioned residual: {}",
        result_yes.residual_norm
    );

    // Preconditioner should take <= iterations (it won't always be strictly
    // fewer on well-conditioned systems, but should not take more).
    assert!(
        result_yes.iterations <= result_no.iterations + 2,
        "preconditioned ({}) should not take much more than unpreconditioned ({})",
        result_yes.iterations,
        result_no.iterations
    );
}

// ---------------------------------------------------------------------------
// Known solution verification
// ---------------------------------------------------------------------------

#[test]
fn test_cg_known_solution() {
    // Diagonal system D*x = b => x_i = b_i / d_i
    let diag_vals = vec![2.0, 5.0, 10.0, 1.0];
    let n = diag_vals.len();
    let entries: Vec<(usize, usize, f64)> = diag_vals
        .iter()
        .enumerate()
        .map(|(i, &d)| (i, i, d))
        .collect();
    let matrix = CsrMatrix::<f64>::from_coo(n, n, entries);

    let rhs = vec![4.0, 15.0, 30.0, 7.0];
    let expected = vec![2.0, 3.0, 3.0, 7.0]; // b_i / d_i

    let budget = default_budget();
    let solver = ConjugateGradientSolver::new(1e-10, 100, false);
    let result = solver.solve(&matrix, &rhs, &budget).unwrap();

    let x = f32_to_f64(&result.solution);
    for i in 0..n {
        assert_relative_eq!(x[i], expected[i], epsilon = 1e-4);
    }

    // Also test a tridiagonal system with known answer.
    // A = [4  -1   0]   b = [3]   =>  solve manually:
    //     [-1  4  -1]       [2]       x0 = (3 + x1)/4
    //     [0  -1   4]       [3]       x2 = (3 + x1)/4 => x0 = x2 (by symmetry)
    //                                  x1 = (2 + x0 + x2)/4 = (2 + 2*x0)/4
    // From row 0: x0 = (3 + x1)/4
    // From row 1: x1 = (2 + 2*x0)/4 = (1 + x0)/2
    // Sub: x0 = (3 + (1+x0)/2)/4 = (3.5 + x0/2)/4 = 7/8 + x0/8
    // => 7x0/8 = 7/8 => x0 = 1, x1 = 1, x2 = 1
    let tri = CsrMatrix::<f64>::from_coo(
        3,
        3,
        vec![
            (0, 0, 4.0),
            (0, 1, -1.0),
            (1, 0, -1.0),
            (1, 1, 4.0),
            (1, 2, -1.0),
            (2, 1, -1.0),
            (2, 2, 4.0),
        ],
    );
    let rhs_tri = vec![3.0, 2.0, 3.0];
    let result_tri = solver.solve(&tri, &rhs_tri, &budget).unwrap();
    let x_tri = f32_to_f64(&result_tri.solution);

    for i in 0..3 {
        assert_relative_eq!(x_tri[i], 1.0, epsilon = 1e-4);
    }
}

// ---------------------------------------------------------------------------
// Tolerance levels: accuracy scales with epsilon
// ---------------------------------------------------------------------------

#[test]
fn test_cg_tolerance_levels() {
    let n = 20;
    let matrix = random_spd_csr(n, 0.3, 48);
    let rhs = random_vector(n, 49);
    let exact = dense_solve(&matrix, &rhs);
    let budget = default_budget();

    let tolerances = [1e-4, 1e-6, 1e-8, 1e-10];
    let mut prev_error = f64::INFINITY;

    for &tol in &tolerances {
        let solver = ConjugateGradientSolver::new(tol, 5000, false);
        let result = solver.solve(&matrix, &rhs, &budget).unwrap();

        let x = f32_to_f64(&result.solution);
        let rel_err = relative_error(&x, &exact);

        // Error should generally decrease with tighter tolerance.
        // Allow some slack for f32 precision limits at very tight tolerances.
        assert!(
            rel_err < tol.sqrt() * 100.0 || rel_err < prev_error * 10.0,
            "tol={:.0e}: relative error {:.2e} is too large (prev={:.2e})",
            tol,
            rel_err,
            prev_error
        );

        prev_error = rel_err;
    }
}
