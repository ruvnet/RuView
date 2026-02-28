//! Shared test helpers for the ruvector-solver integration test suite.
//!
//! Provides deterministic random matrix generators, dense reference solvers,
//! and floating-point comparison utilities used across all test modules.

use ruvector_solver::types::CsrMatrix;

// ---------------------------------------------------------------------------
// Random number generator (simple LCG for deterministic reproducibility)
// ---------------------------------------------------------------------------

/// A minimal linear congruential generator for deterministic test data.
///
/// Uses the Numerical Recipes LCG parameters. Not cryptographically secure,
/// but perfectly adequate for generating reproducible test matrices.
pub struct Lcg {
    state: u64,
}

impl Lcg {
    /// Create a new LCG with the given seed.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Generate the next u64 value.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    /// Generate a uniform f64 in [0, 1).
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Generate a uniform f64 in [lo, hi).
    pub fn next_f64_range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }
}

// ---------------------------------------------------------------------------
// Matrix generators
// ---------------------------------------------------------------------------

/// Generate a random diagonally dominant CSR matrix of dimension `n`.
///
/// Each row has approximately `density * n` non-zero off-diagonal entries
/// (at least 1). The diagonal entry is set to `1 + sum_of_abs_off_diag`
/// to guarantee strict diagonal dominance.
///
/// The resulting matrix is suitable for Neumann and Jacobi solvers.
pub fn random_diag_dominant_csr(n: usize, density: f64, seed: u64) -> CsrMatrix<f64> {
    let mut rng = Lcg::new(seed);
    let mut entries: Vec<(usize, usize, f64)> = Vec::new();

    for i in 0..n {
        let mut off_diag_sum = 0.0f64;

        for j in 0..n {
            if i == j {
                continue;
            }
            if rng.next_f64() < density {
                let val = rng.next_f64_range(-1.0, 1.0);
                entries.push((i, j, val));
                off_diag_sum += val.abs();
            }
        }

        // Ensure at least one off-diagonal entry per row for non-trivial testing.
        if off_diag_sum == 0.0 && n > 1 {
            let j = (i + 1) % n;
            let val = rng.next_f64_range(0.1, 0.5);
            entries.push((i, j, val));
            off_diag_sum = val;
        }

        // Diagonal: strictly dominant.
        let diag_val = off_diag_sum + 1.0 + rng.next_f64();
        entries.push((i, i, diag_val));
    }

    CsrMatrix::<f64>::from_coo(n, n, entries)
}

/// Generate a random graph Laplacian CSR matrix of dimension `n`.
///
/// A graph Laplacian `L = D - A` where:
/// - `A` is the adjacency matrix of a random undirected graph.
/// - `D` is the degree matrix.
/// - Each row sums to zero (L * ones = 0).
///
/// The resulting matrix is symmetric positive semi-definite.
pub fn random_laplacian_csr(n: usize, density: f64, seed: u64) -> CsrMatrix<f64> {
    let mut rng = Lcg::new(seed);

    // Build symmetric adjacency: for i < j, randomly include edge (i,j).
    let mut adj = vec![vec![0.0f64; n]; n];

    for i in 0..n {
        for j in (i + 1)..n {
            if rng.next_f64() < density {
                let weight = rng.next_f64_range(0.1, 2.0);
                adj[i][j] = weight;
                adj[j][i] = weight;
            }
        }
    }

    // Ensure the graph is connected: add a path 0-1-2-...-n-1.
    for i in 0..n.saturating_sub(1) {
        if adj[i][i + 1] == 0.0 {
            let weight = rng.next_f64_range(0.1, 1.0);
            adj[i][i + 1] = weight;
            adj[i + 1][i] = weight;
        }
    }

    // Build Laplacian: L[i][j] = -A[i][j] for i != j, L[i][i] = sum_j A[i][j].
    let mut entries: Vec<(usize, usize, f64)> = Vec::new();

    for i in 0..n {
        let mut degree = 0.0f64;
        for j in 0..n {
            if i != j && adj[i][j] != 0.0 {
                entries.push((i, j, -adj[i][j]));
                degree += adj[i][j];
            }
        }
        entries.push((i, i, degree));
    }

    CsrMatrix::<f64>::from_coo(n, n, entries)
}

/// Generate a random SPD (symmetric positive definite) matrix.
///
/// Constructs `A = B^T B + epsilon * I` where `B` has random entries,
/// guaranteeing positive definiteness.
pub fn random_spd_csr(n: usize, density: f64, seed: u64) -> CsrMatrix<f64> {
    let mut rng = Lcg::new(seed);

    // Build a random dense matrix B, then compute A = B^T B + eps * I.
    // For efficiency with CSR, we do this differently: build a sparse
    // symmetric matrix and add a diagonal shift.
    let mut dense = vec![vec![0.0f64; n]; n];

    for i in 0..n {
        for j in i..n {
            if i == j || rng.next_f64() < density {
                let val = rng.next_f64_range(-1.0, 1.0);
                dense[i][j] += val;
                if i != j {
                    dense[j][i] += val;
                }
            }
        }
    }

    // Compute A = M^T M where M = dense (makes it PSD).
    let mut a = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for j in 0..n {
            let mut sum = 0.0;
            for k in 0..n {
                sum += dense[k][i] * dense[k][j];
            }
            a[i][j] = sum;
        }
    }

    // Add diagonal shift to ensure positive definiteness.
    for i in 0..n {
        a[i][i] += 1.0;
    }

    // Convert to COO.
    let mut entries: Vec<(usize, usize, f64)> = Vec::new();
    for i in 0..n {
        for j in 0..n {
            if a[i][j].abs() > 1e-15 {
                entries.push((i, j, a[i][j]));
            }
        }
    }

    CsrMatrix::<f64>::from_coo(n, n, entries)
}

/// Generate a deterministic random vector of length `n`.
pub fn random_vector(n: usize, seed: u64) -> Vec<f64> {
    let mut rng = Lcg::new(seed);
    (0..n).map(|_| rng.next_f64_range(-1.0, 1.0)).collect()
}

/// Build a simple undirected graph as a CSR adjacency matrix.
///
/// Each entry `(u, v)` in `edges` creates entries `A[u][v] = 1` and
/// `A[v][u] = 1`.
pub fn adjacency_from_edges(n: usize, edges: &[(usize, usize)]) -> CsrMatrix<f64> {
    let mut entries: Vec<(usize, usize, f64)> = Vec::new();
    for &(u, v) in edges {
        entries.push((u, v, 1.0));
        if u != v {
            entries.push((v, u, 1.0));
        }
    }
    CsrMatrix::<f64>::from_coo(n, n, entries)
}

// ---------------------------------------------------------------------------
// Dense reference solver
// ---------------------------------------------------------------------------

/// Solve `Ax = b` using dense Gaussian elimination with partial pivoting.
///
/// This is an O(n^3) reference solver used only for small test problems
/// to verify iterative solver accuracy.
///
/// # Panics
///
/// Panics if the matrix is singular or dimensions are inconsistent.
pub fn dense_solve(matrix: &CsrMatrix<f64>, rhs: &[f64]) -> Vec<f64> {
    let n = matrix.rows;
    assert_eq!(n, matrix.cols, "dense_solve requires a square matrix");
    assert_eq!(rhs.len(), n, "rhs length must match matrix dimension");

    // Convert CSR to dense augmented matrix [A | b].
    let mut aug = vec![vec![0.0f64; n + 1]; n];
    for i in 0..n {
        aug[i][n] = rhs[i];
        let start = matrix.row_ptr[i];
        let end = matrix.row_ptr[i + 1];
        for idx in start..end {
            let j = matrix.col_indices[idx];
            aug[i][j] = matrix.values[idx];
        }
    }

    // Forward elimination with partial pivoting.
    for col in 0..n {
        // Find pivot.
        let mut max_row = col;
        let mut max_val = aug[col][col].abs();
        for row in (col + 1)..n {
            if aug[row][col].abs() > max_val {
                max_val = aug[row][col].abs();
                max_row = row;
            }
        }
        assert!(max_val > 1e-15, "matrix is singular or near-singular");
        aug.swap(col, max_row);

        // Eliminate.
        let pivot = aug[col][col];
        for row in (col + 1)..n {
            let factor = aug[row][col] / pivot;
            for j in col..=n {
                aug[row][j] -= factor * aug[col][j];
            }
        }
    }

    // Back substitution.
    let mut x = vec![0.0f64; n];
    for i in (0..n).rev() {
        let mut sum = aug[i][n];
        for j in (i + 1)..n {
            sum -= aug[i][j] * x[j];
        }
        x[i] = sum / aug[i][i];
    }

    x
}

// ---------------------------------------------------------------------------
// Floating-point comparison utilities
// ---------------------------------------------------------------------------

/// Compute the L2 norm of a vector.
pub fn l2_norm(v: &[f64]) -> f64 {
    v.iter().map(|&x| x * x).sum::<f64>().sqrt()
}

/// Compute the L2 distance between two vectors.
pub fn l2_distance(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len(), "vectors must have same length");
    a.iter()
        .zip(b.iter())
        .map(|(&ai, &bi)| (ai - bi) * (ai - bi))
        .sum::<f64>()
        .sqrt()
}

/// Compute the relative error ||approx - exact|| / ||exact||.
///
/// Returns absolute error if the exact solution has zero norm.
pub fn relative_error(approx: &[f64], exact: &[f64]) -> f64 {
    let exact_norm = l2_norm(exact);
    let error = l2_distance(approx, exact);
    if exact_norm > 1e-15 {
        error / exact_norm
    } else {
        error
    }
}

/// Compute the residual `b - A*x` for a sparse system.
pub fn compute_residual(matrix: &CsrMatrix<f64>, x: &[f64], rhs: &[f64]) -> Vec<f64> {
    let n = matrix.rows;
    let mut ax = vec![0.0f64; n];
    matrix.spmv(x, &mut ax);
    (0..n).map(|i| rhs[i] - ax[i]).collect()
}

/// Convert an f32 solution vector to f64 for comparison.
pub fn f32_to_f64(v: &[f32]) -> Vec<f64> {
    v.iter().map(|&x| x as f64).collect()
}
