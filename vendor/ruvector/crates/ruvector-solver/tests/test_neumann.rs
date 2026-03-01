//! Integration tests for the Neumann series solver.
//!
//! The Neumann solver solves Ax = b by iterating x_{k+1} = b + (I - A) x_k.
//! Convergence requires spectral radius rho(I - A) < 1, which is guaranteed
//! for diagonally dominant systems.

mod helpers;

use approx::assert_relative_eq;
use ruvector_solver::error::SolverError;
use ruvector_solver::neumann::NeumannSolver;
use ruvector_solver::traits::SolverEngine;
use ruvector_solver::types::{Algorithm, ComputeBudget, CsrMatrix};

use helpers::{
    compute_residual, dense_solve, f32_to_f64, l2_norm, random_diag_dominant_csr, random_vector,
    relative_error,
};

// ---------------------------------------------------------------------------
// Helper: call solver via the SolverEngine trait (f64 interface)
// ---------------------------------------------------------------------------

fn solve_via_trait(
    solver: &NeumannSolver,
    matrix: &CsrMatrix<f64>,
    rhs: &[f64],
    budget: &ComputeBudget,
) -> Result<ruvector_solver::types::SolverResult, SolverError> {
    SolverEngine::solve(solver, matrix, rhs, budget)
}

// ---------------------------------------------------------------------------
// Solve a diagonally dominant system, verify ||Ax - b|| < eps
// ---------------------------------------------------------------------------

#[test]
fn test_neumann_diagonal_dominant() {
    let n = 20;
    let matrix = random_diag_dominant_csr(n, 0.3, 42);
    let rhs = random_vector(n, 43);
    let budget = ComputeBudget::default();

    let solver = NeumannSolver::new(1e-8, 500);
    let result = solve_via_trait(&solver, &matrix, &rhs, &budget).unwrap();

    assert!(
        result.residual_norm < 1e-6,
        "residual too large: {}",
        result.residual_norm
    );

    // Double-check by computing residual independently.
    let x = f32_to_f64(&result.solution);
    let residual = compute_residual(&matrix, &x, &rhs);
    let resid_norm = l2_norm(&residual);
    assert!(
        resid_norm < 1e-4,
        "independent residual check failed: {}",
        resid_norm
    );

    // Compare with dense solve.
    let exact = dense_solve(&matrix, &rhs);
    let rel_err = relative_error(&x, &exact);
    assert!(rel_err < 1e-3, "relative error vs dense solve: {}", rel_err);
}

// ---------------------------------------------------------------------------
// Verify geometric convergence rate
// ---------------------------------------------------------------------------

#[test]
fn test_neumann_convergence_rate() {
    let n = 15;
    let matrix = random_diag_dominant_csr(n, 0.2, 44);
    let rhs = random_vector(n, 45);
    let budget = ComputeBudget::default();

    let solver = NeumannSolver::new(1e-12, 500);
    let result = solve_via_trait(&solver, &matrix, &rhs, &budget).unwrap();

    // The convergence history should show monotonic decrease (geometric).
    let history = &result.convergence_history;
    assert!(
        history.len() >= 3,
        "need at least 3 iterations for rate check"
    );

    // Check that residual decreases monotonically for at least the first
    // several iterations (allowing a small tolerance for floating point).
    let mut decreasing_count = 0;
    for w in history.windows(2) {
        if w[1].residual_norm < w[0].residual_norm * 1.01 {
            decreasing_count += 1;
        }
    }
    let decrease_ratio = decreasing_count as f64 / (history.len() - 1) as f64;
    assert!(
        decrease_ratio > 0.8,
        "expected mostly decreasing residuals, got {:.0}% decreasing",
        decrease_ratio * 100.0
    );

    // Estimate the convergence factor from the last few iterations.
    if history.len() >= 4 {
        let n_hist = history.len();
        let r_late = history[n_hist - 1].residual_norm;
        let r_early = history[n_hist - 4].residual_norm;
        if r_early > 1e-15 {
            let avg_factor = (r_late / r_early).powf(1.0 / 3.0);
            assert!(
                avg_factor < 1.0,
                "convergence factor should be < 1, got {}",
                avg_factor
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Verify rejection when spectral radius >= 1
// ---------------------------------------------------------------------------

#[test]
fn test_neumann_spectral_radius_check() {
    // Build a matrix where off-diagonal entries dominate the diagonal,
    // giving spectral radius >= 1. The matrix:
    // [1  2]
    // [2  1]
    // has off-diag sum > diag for each row.
    let matrix = CsrMatrix::<f64>::from_coo(
        2,
        2,
        vec![(0, 0, 1.0), (0, 1, 2.0), (1, 0, 2.0), (1, 1, 1.0)],
    );
    let rhs = vec![1.0, 1.0];
    let budget = ComputeBudget::default();

    let solver = NeumannSolver::new(1e-8, 100);
    let result = solve_via_trait(&solver, &matrix, &rhs, &budget);

    match result {
        Err(SolverError::SpectralRadiusExceeded {
            spectral_radius,
            limit,
            algorithm,
        }) => {
            assert!(spectral_radius >= 1.0);
            assert_relative_eq!(limit, 1.0, epsilon = 1e-12);
            assert_eq!(algorithm, Algorithm::Neumann);
        }
        Err(SolverError::NumericalInstability { .. }) => {
            // Also acceptable: the solver might detect NaN divergence
            // before the spectral radius check catches it.
        }
        other => panic!(
            "expected SpectralRadiusExceeded or NumericalInstability, got {:?}",
            other
        ),
    }
}

// ---------------------------------------------------------------------------
// Identity system: Ix = b should converge in 1 iteration
// ---------------------------------------------------------------------------

#[test]
fn test_neumann_identity_system() {
    for n in [1, 5, 20] {
        let matrix = CsrMatrix::<f64>::identity(n);
        let rhs: Vec<f64> = (0..n).map(|i| (i as f64 + 1.0) * 0.3).collect();
        let budget = ComputeBudget::default();

        let solver = NeumannSolver::new(1e-10, 100);
        let result = solve_via_trait(&solver, &matrix, &rhs, &budget).unwrap();

        // Solution should equal rhs for identity matrix.
        let x = f32_to_f64(&result.solution);
        for i in 0..n {
            assert_relative_eq!(x[i], rhs[i], epsilon = 1e-5);
        }

        // Should converge in exactly 1 iteration for identity.
        assert_eq!(
            result.iterations, 1,
            "identity system should converge in 1 iteration, got {}",
            result.iterations
        );
        assert_eq!(result.algorithm, Algorithm::Neumann);
    }
}

// ---------------------------------------------------------------------------
// Manually constructed system with known solution
// ---------------------------------------------------------------------------

#[test]
fn test_neumann_known_solution() {
    // System:
    //  [3  -1   0] [x0]   [2]
    //  [-1  3  -1] [x1] = [0]
    //  [0  -1   3] [x2]   [2]
    //
    // By symmetry, x0 = x2. From row 0: 3*x0 - x1 = 2.
    // From row 1: -x0 + 3*x1 - x2 = 0 => -2*x0 + 3*x1 = 0 => x1 = 2*x0/3.
    // Substituting: 3*x0 - 2*x0/3 = 2 => (9 - 2)/3 * x0 = 2 => x0 = 6/7.
    // x1 = 4/7, x2 = 6/7.
    let matrix = CsrMatrix::<f64>::from_coo(
        3,
        3,
        vec![
            (0, 0, 3.0),
            (0, 1, -1.0),
            (1, 0, -1.0),
            (1, 1, 3.0),
            (1, 2, -1.0),
            (2, 1, -1.0),
            (2, 2, 3.0),
        ],
    );
    let rhs = vec![2.0, 0.0, 2.0];
    let budget = ComputeBudget::default();

    let solver = NeumannSolver::new(1e-10, 500);
    let result = solve_via_trait(&solver, &matrix, &rhs, &budget).unwrap();

    let x = f32_to_f64(&result.solution);
    let expected = [6.0 / 7.0, 4.0 / 7.0, 6.0 / 7.0];

    for i in 0..3 {
        assert_relative_eq!(x[i], expected[i], epsilon = 1e-4);
    }
}
