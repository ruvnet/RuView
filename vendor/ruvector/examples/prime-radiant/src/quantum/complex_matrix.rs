//! Complex Matrix and Vector Operations
//!
//! Provides fundamental complex linear algebra operations for quantum computing.
//! Uses `num-complex` for complex number arithmetic with f64 precision.

use std::ops::{Add, Mul, Sub};
use ruvector_solver::types::CsrMatrix;

/// Complex number type alias using f64 precision
pub type Complex64 = num_complex::Complex<f64>;

/// Complex vector type
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexVector {
    /// Vector elements
    pub data: Vec<Complex64>,
}

impl ComplexVector {
    /// Create a new complex vector from components
    pub fn new(data: Vec<Complex64>) -> Self {
        Self { data }
    }

    /// Create a zero vector of given dimension
    pub fn zeros(dim: usize) -> Self {
        Self {
            data: vec![Complex64::new(0.0, 0.0); dim],
        }
    }

    /// Create a vector from real components
    pub fn from_real(reals: &[f64]) -> Self {
        Self {
            data: reals.iter().map(|&r| Complex64::new(r, 0.0)).collect(),
        }
    }

    /// Create a computational basis state |i⟩
    pub fn basis_state(dim: usize, index: usize) -> Self {
        let mut data = vec![Complex64::new(0.0, 0.0); dim];
        if index < dim {
            data[index] = Complex64::new(1.0, 0.0);
        }
        Self { data }
    }

    /// Dimension of the vector
    pub fn dim(&self) -> usize {
        self.data.len()
    }

    /// Compute the L2 norm (Euclidean norm)
    pub fn norm(&self) -> f64 {
        self.norm_squared().sqrt()
    }

    /// Compute the squared norm
    pub fn norm_squared(&self) -> f64 {
        self.data.iter().map(|c| c.norm_sqr()).sum()
    }

    /// Normalize the vector in place
    pub fn normalize(&mut self) {
        let n = self.norm();
        if n > 1e-15 {
            for c in &mut self.data {
                *c /= n;
            }
        }
    }

    /// Return a normalized copy
    pub fn normalized(&self) -> Self {
        let mut result = self.clone();
        result.normalize();
        result
    }

    /// Inner product ⟨self|other⟩ = self† · other
    pub fn inner(&self, other: &ComplexVector) -> Complex64 {
        assert_eq!(self.dim(), other.dim(), "Dimension mismatch in inner product");
        self.data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| a.conj() * b)
            .sum()
    }

    /// Outer product |self⟩⟨other| = self ⊗ other†
    pub fn outer(&self, other: &ComplexVector) -> ComplexMatrix {
        let rows = self.dim();
        let cols = other.dim();
        let mut data = vec![Complex64::new(0.0, 0.0); rows * cols];

        for i in 0..rows {
            for j in 0..cols {
                data[i * cols + j] = self.data[i] * other.data[j].conj();
            }
        }

        ComplexMatrix { data, rows, cols }
    }

    /// Tensor product |self⟩ ⊗ |other⟩
    pub fn tensor(&self, other: &ComplexVector) -> Self {
        let mut result = Vec::with_capacity(self.dim() * other.dim());
        for a in &self.data {
            for b in &other.data {
                result.push(a * b);
            }
        }
        Self { data: result }
    }

    /// Element-wise conjugate
    pub fn conjugate(&self) -> Self {
        Self {
            data: self.data.iter().map(|c| c.conj()).collect(),
        }
    }

    /// Scale the vector
    pub fn scale(&self, factor: Complex64) -> Self {
        Self {
            data: self.data.iter().map(|c| c * factor).collect(),
        }
    }

    /// Add two vectors
    pub fn add(&self, other: &ComplexVector) -> Self {
        assert_eq!(self.dim(), other.dim(), "Dimension mismatch in addition");
        Self {
            data: self
                .data
                .iter()
                .zip(other.data.iter())
                .map(|(a, b)| a + b)
                .collect(),
        }
    }

    /// Subtract two vectors
    pub fn sub(&self, other: &ComplexVector) -> Self {
        assert_eq!(self.dim(), other.dim(), "Dimension mismatch in subtraction");
        Self {
            data: self
                .data
                .iter()
                .zip(other.data.iter())
                .map(|(a, b)| a - b)
                .collect(),
        }
    }
}

impl Add for &ComplexVector {
    type Output = ComplexVector;

    fn add(self, other: &ComplexVector) -> ComplexVector {
        ComplexVector::add(self, other)
    }
}

impl Sub for &ComplexVector {
    type Output = ComplexVector;

    fn sub(self, other: &ComplexVector) -> ComplexVector {
        ComplexVector::sub(self, other)
    }
}

/// Complex matrix for quantum operations
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexMatrix {
    /// Row-major data storage
    pub data: Vec<Complex64>,
    /// Number of rows
    pub rows: usize,
    /// Number of columns
    pub cols: usize,
}

impl ComplexMatrix {
    /// Create a new matrix from row-major data
    pub fn new(data: Vec<Complex64>, rows: usize, cols: usize) -> Self {
        assert_eq!(data.len(), rows * cols, "Data length must match dimensions");
        Self { data, rows, cols }
    }

    /// Create a zero matrix
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            data: vec![Complex64::new(0.0, 0.0); rows * cols],
            rows,
            cols,
        }
    }

    /// Create an identity matrix
    pub fn identity(n: usize) -> Self {
        let mut data = vec![Complex64::new(0.0, 0.0); n * n];
        for i in 0..n {
            data[i * n + i] = Complex64::new(1.0, 0.0);
        }
        Self {
            data,
            rows: n,
            cols: n,
        }
    }

    /// Create a matrix from real values
    pub fn from_real(reals: &[f64], rows: usize, cols: usize) -> Self {
        assert_eq!(reals.len(), rows * cols, "Data length must match dimensions");
        Self {
            data: reals.iter().map(|&r| Complex64::new(r, 0.0)).collect(),
            rows,
            cols,
        }
    }

    /// Create a diagonal matrix from a vector
    pub fn diagonal(diag: &[Complex64]) -> Self {
        let n = diag.len();
        let mut data = vec![Complex64::new(0.0, 0.0); n * n];
        for (i, &val) in diag.iter().enumerate() {
            data[i * n + i] = val;
        }
        Self {
            data,
            rows: n,
            cols: n,
        }
    }

    /// Get element at (row, col)
    pub fn get(&self, row: usize, col: usize) -> Complex64 {
        assert!(row < self.rows && col < self.cols, "Index out of bounds");
        self.data[row * self.cols + col]
    }

    /// Set element at (row, col)
    pub fn set(&mut self, row: usize, col: usize, value: Complex64) {
        assert!(row < self.rows && col < self.cols, "Index out of bounds");
        self.data[row * self.cols + col] = value;
    }

    /// Check if matrix is square
    pub fn is_square(&self) -> bool {
        self.rows == self.cols
    }

    /// Compute the trace (sum of diagonal elements)
    pub fn trace(&self) -> Complex64 {
        assert!(self.is_square(), "Trace requires square matrix");
        (0..self.rows).map(|i| self.get(i, i)).sum()
    }

    /// Compute the conjugate transpose (Hermitian adjoint) A†
    pub fn adjoint(&self) -> Self {
        let mut data = vec![Complex64::new(0.0, 0.0); self.rows * self.cols];
        for i in 0..self.rows {
            for j in 0..self.cols {
                data[j * self.rows + i] = self.get(i, j).conj();
            }
        }
        Self {
            data,
            rows: self.cols,
            cols: self.rows,
        }
    }

    /// Compute the transpose
    pub fn transpose(&self) -> Self {
        let mut data = vec![Complex64::new(0.0, 0.0); self.rows * self.cols];
        for i in 0..self.rows {
            for j in 0..self.cols {
                data[j * self.rows + i] = self.get(i, j);
            }
        }
        Self {
            data,
            rows: self.cols,
            cols: self.rows,
        }
    }

    /// Check if matrix is Hermitian (A = A†)
    pub fn is_hermitian(&self, tolerance: f64) -> bool {
        if !self.is_square() {
            return false;
        }
        for i in 0..self.rows {
            for j in 0..=i {
                let diff = (self.get(i, j) - self.get(j, i).conj()).norm();
                if diff > tolerance {
                    return false;
                }
            }
        }
        true
    }

    /// Check if all entries have negligible imaginary parts
    pub fn is_real_valued(&self, tolerance: f64) -> bool {
        self.data.iter().all(|c| c.im.abs() <= tolerance)
    }

    /// Convert a real-valued ComplexMatrix to CsrMatrix<f64>.
    /// Returns None if any entry has a significant imaginary part.
    pub fn to_csr_real(&self, tolerance: f64) -> Option<CsrMatrix<f64>> {
        if !self.is_real_valued(tolerance) {
            return None;
        }
        let entries: Vec<(usize, usize, f64)> = (0..self.rows)
            .flat_map(|i| {
                (0..self.cols)
                    .filter_map(move |j| {
                        let val = self.get(i, j).re;
                        if val.abs() > tolerance {
                            Some((i, j, val))
                        } else {
                            None
                        }
                    })
            })
            .collect();
        Some(CsrMatrix::<f64>::from_coo(self.rows, self.cols, entries))
    }

    /// Check if matrix is unitary (A†A = I)
    pub fn is_unitary(&self, tolerance: f64) -> bool {
        if !self.is_square() {
            return false;
        }
        let product = self.adjoint().matmul(self);
        let identity = ComplexMatrix::identity(self.rows);

        for i in 0..self.rows {
            for j in 0..self.cols {
                let diff = (product.get(i, j) - identity.get(i, j)).norm();
                if diff > tolerance {
                    return false;
                }
            }
        }
        true
    }

    /// Matrix-vector multiplication
    pub fn matvec(&self, v: &ComplexVector) -> ComplexVector {
        assert_eq!(self.cols, v.dim(), "Dimension mismatch in matrix-vector product");
        let mut result = Vec::with_capacity(self.rows);
        for i in 0..self.rows {
            let mut sum = Complex64::new(0.0, 0.0);
            for j in 0..self.cols {
                sum += self.get(i, j) * v.data[j];
            }
            result.push(sum);
        }
        ComplexVector::new(result)
    }

    /// Matrix-matrix multiplication
    pub fn matmul(&self, other: &ComplexMatrix) -> Self {
        assert_eq!(
            self.cols, other.rows,
            "Dimension mismatch in matrix multiplication"
        );
        let mut data = vec![Complex64::new(0.0, 0.0); self.rows * other.cols];
        for i in 0..self.rows {
            for j in 0..other.cols {
                let mut sum = Complex64::new(0.0, 0.0);
                for k in 0..self.cols {
                    sum += self.get(i, k) * other.get(k, j);
                }
                data[i * other.cols + j] = sum;
            }
        }
        Self {
            data,
            rows: self.rows,
            cols: other.cols,
        }
    }

    /// Scale the matrix by a complex factor
    pub fn scale(&self, factor: Complex64) -> Self {
        Self {
            data: self.data.iter().map(|c| c * factor).collect(),
            rows: self.rows,
            cols: self.cols,
        }
    }

    /// Add two matrices
    pub fn add(&self, other: &ComplexMatrix) -> Self {
        assert_eq!(
            (self.rows, self.cols),
            (other.rows, other.cols),
            "Dimension mismatch in matrix addition"
        );
        Self {
            data: self
                .data
                .iter()
                .zip(other.data.iter())
                .map(|(a, b)| a + b)
                .collect(),
            rows: self.rows,
            cols: self.cols,
        }
    }

    /// Subtract two matrices
    pub fn sub(&self, other: &ComplexMatrix) -> Self {
        assert_eq!(
            (self.rows, self.cols),
            (other.rows, other.cols),
            "Dimension mismatch in matrix subtraction"
        );
        Self {
            data: self
                .data
                .iter()
                .zip(other.data.iter())
                .map(|(a, b)| a - b)
                .collect(),
            rows: self.rows,
            cols: self.cols,
        }
    }

    /// Tensor (Kronecker) product A ⊗ B
    pub fn tensor(&self, other: &ComplexMatrix) -> Self {
        let new_rows = self.rows * other.rows;
        let new_cols = self.cols * other.cols;
        let mut data = vec![Complex64::new(0.0, 0.0); new_rows * new_cols];

        for i in 0..self.rows {
            for j in 0..self.cols {
                let a_ij = self.get(i, j);
                for k in 0..other.rows {
                    for l in 0..other.cols {
                        let row = i * other.rows + k;
                        let col = j * other.cols + l;
                        data[row * new_cols + col] = a_ij * other.get(k, l);
                    }
                }
            }
        }

        Self {
            data,
            rows: new_rows,
            cols: new_cols,
        }
    }

    /// Compute the Frobenius norm ||A||_F = sqrt(Tr(A†A))
    pub fn frobenius_norm(&self) -> f64 {
        self.data.iter().map(|c| c.norm_sqr()).sum::<f64>().sqrt()
    }

    /// Compute partial trace over subsystem B for a bipartite system AB
    /// Assumes dimensions: total = dim_a * dim_b, traces out subsystem B
    pub fn partial_trace_b(&self, dim_a: usize, dim_b: usize) -> Self {
        assert!(self.is_square(), "Partial trace requires square matrix");
        assert_eq!(self.rows, dim_a * dim_b, "Dimensions must match");

        let mut result = Self::zeros(dim_a, dim_a);

        for i in 0..dim_a {
            for j in 0..dim_a {
                let mut sum = Complex64::new(0.0, 0.0);
                for k in 0..dim_b {
                    let row = i * dim_b + k;
                    let col = j * dim_b + k;
                    sum += self.get(row, col);
                }
                result.set(i, j, sum);
            }
        }

        result
    }

    /// Compute partial trace over subsystem A for a bipartite system AB
    pub fn partial_trace_a(&self, dim_a: usize, dim_b: usize) -> Self {
        assert!(self.is_square(), "Partial trace requires square matrix");
        assert_eq!(self.rows, dim_a * dim_b, "Dimensions must match");

        let mut result = Self::zeros(dim_b, dim_b);

        for i in 0..dim_b {
            for j in 0..dim_b {
                let mut sum = Complex64::new(0.0, 0.0);
                for k in 0..dim_a {
                    let row = k * dim_b + i;
                    let col = k * dim_b + j;
                    sum += self.get(row, col);
                }
                result.set(i, j, sum);
            }
        }

        result
    }

    /// Compute eigenvalues of a real symmetric matrix using the Jacobi method.
    ///
    /// This is significantly more numerically stable than power iteration + deflation,
    /// especially for matrices with clustered eigenvalues (common in density matrices).
    /// Uses CsrMatrix for efficient matrix operations when the matrix is sparse.
    fn eigenvalues_real_symmetric(&self, max_iterations: usize, tolerance: f64) -> Vec<Complex64> {
        let n = self.rows;

        // Work with a real copy (row-major f64)
        let mut a: Vec<Vec<f64>> = (0..n)
            .map(|i| (0..n).map(|j| self.get(i, j).re).collect())
            .collect();

        // Jacobi eigenvalue algorithm: repeatedly zero off-diagonal elements
        for _ in 0..max_iterations * n {
            // Find largest off-diagonal element
            let mut max_val = 0.0f64;
            let mut p = 0;
            let mut q = 1;

            for i in 0..n {
                for j in (i + 1)..n {
                    if a[i][j].abs() > max_val {
                        max_val = a[i][j].abs();
                        p = i;
                        q = j;
                    }
                }
            }

            // Check convergence
            if max_val < tolerance {
                break;
            }

            // Compute Jacobi rotation angle
            let theta = if (a[p][p] - a[q][q]).abs() < 1e-15 {
                std::f64::consts::FRAC_PI_4
            } else {
                0.5 * (2.0 * a[p][q] / (a[p][p] - a[q][q])).atan()
            };

            let c = theta.cos();
            let s = theta.sin();

            // Apply Jacobi rotation: A' = J^T A J
            // Update rows p and q
            let mut new_row_p = vec![0.0; n];
            let mut new_row_q = vec![0.0; n];

            for j in 0..n {
                new_row_p[j] = c * a[p][j] + s * a[q][j];
                new_row_q[j] = -s * a[p][j] + c * a[q][j];
            }

            for j in 0..n {
                a[p][j] = new_row_p[j];
                a[q][j] = new_row_q[j];
            }

            // Update columns p and q
            for i in 0..n {
                let aip = a[i][p];
                let aiq = a[i][q];
                a[i][p] = c * aip + s * aiq;
                a[i][q] = -s * aip + c * aiq;
            }
        }

        // Eigenvalues are on the diagonal
        let mut eigenvalues: Vec<Complex64> = (0..n)
            .map(|i| Complex64::new(a[i][i], 0.0))
            .collect();

        // Sort by magnitude (descending)
        eigenvalues.sort_by(|a, b| b.norm().partial_cmp(&a.norm()).unwrap_or(std::cmp::Ordering::Equal));

        eigenvalues
    }

    /// Compute eigenvalues using QR iteration (simplified version)
    /// Returns eigenvalues in descending order of magnitude
    pub fn eigenvalues(&self, max_iterations: usize, tolerance: f64) -> Vec<Complex64> {
        assert!(self.is_square(), "Eigenvalues require square matrix");

        let n = self.rows;
        if n == 0 {
            return vec![];
        }
        if n == 1 {
            return vec![self.get(0, 0)];
        }

        // For 2x2 matrices, use closed-form solution
        if n == 2 {
            let a = self.get(0, 0);
            let b = self.get(0, 1);
            let c = self.get(1, 0);
            let d = self.get(1, 1);

            let trace = a + d;
            let det = a * d - b * c;
            let discriminant = trace * trace - Complex64::new(4.0, 0.0) * det;
            let sqrt_disc = discriminant.sqrt();

            let lambda1 = (trace + sqrt_disc) / Complex64::new(2.0, 0.0);
            let lambda2 = (trace - sqrt_disc) / Complex64::new(2.0, 0.0);

            return vec![lambda1, lambda2];
        }

        // For Hermitian + real-valued matrices, use Jacobi (much more stable)
        if self.is_hermitian(tolerance) && self.is_real_valued(tolerance) {
            return self.eigenvalues_real_symmetric(max_iterations, tolerance);
        }

        // For larger matrices, use power iteration to find dominant eigenvalue
        // This is a simplified implementation
        let mut eigenvalues = Vec::with_capacity(n);
        let mut working_matrix = self.clone();

        for _ in 0..n.min(max_iterations) {
            // Power iteration to find largest eigenvalue
            let mut v = ComplexVector::new(vec![Complex64::new(1.0, 0.0); working_matrix.rows]);
            v.normalize();

            let mut eigenvalue = Complex64::new(0.0, 0.0);

            for _ in 0..max_iterations {
                let new_v = working_matrix.matvec(&v);
                let new_eigenvalue = v.inner(&new_v);

                if (new_eigenvalue - eigenvalue).norm() < tolerance {
                    eigenvalue = new_eigenvalue;
                    break;
                }

                eigenvalue = new_eigenvalue;
                v = new_v.normalized();
            }

            eigenvalues.push(eigenvalue);

            // Deflate matrix (simplified)
            if working_matrix.rows > 1 {
                working_matrix = working_matrix.sub(&v.outer(&v).scale(eigenvalue));
            }
        }

        // Sort by magnitude (descending)
        eigenvalues.sort_by(|a, b| b.norm().partial_cmp(&a.norm()).unwrap_or(std::cmp::Ordering::Equal));

        eigenvalues
    }
}

impl Mul for &ComplexMatrix {
    type Output = ComplexMatrix;

    fn mul(self, other: &ComplexMatrix) -> ComplexMatrix {
        self.matmul(other)
    }
}

impl Add for &ComplexMatrix {
    type Output = ComplexMatrix;

    fn add(self, other: &ComplexMatrix) -> ComplexMatrix {
        ComplexMatrix::add(self, other)
    }
}

impl Sub for &ComplexMatrix {
    type Output = ComplexMatrix;

    fn sub(self, other: &ComplexMatrix) -> ComplexMatrix {
        ComplexMatrix::sub(self, other)
    }
}

/// Common quantum gates as matrices
pub mod gates {
    use super::*;

    /// Pauli X gate (NOT gate)
    pub fn pauli_x() -> ComplexMatrix {
        ComplexMatrix::new(
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            2,
            2,
        )
    }

    /// Pauli Y gate
    pub fn pauli_y() -> ComplexMatrix {
        ComplexMatrix::new(
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, -1.0),
                Complex64::new(0.0, 1.0),
                Complex64::new(0.0, 0.0),
            ],
            2,
            2,
        )
    }

    /// Pauli Z gate
    pub fn pauli_z() -> ComplexMatrix {
        ComplexMatrix::new(
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(-1.0, 0.0),
            ],
            2,
            2,
        )
    }

    /// Hadamard gate
    pub fn hadamard() -> ComplexMatrix {
        let s = 1.0 / 2.0_f64.sqrt();
        ComplexMatrix::new(
            vec![
                Complex64::new(s, 0.0),
                Complex64::new(s, 0.0),
                Complex64::new(s, 0.0),
                Complex64::new(-s, 0.0),
            ],
            2,
            2,
        )
    }

    /// Phase gate S = sqrt(Z)
    pub fn phase() -> ComplexMatrix {
        ComplexMatrix::new(
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 1.0),
            ],
            2,
            2,
        )
    }

    /// T gate (pi/8 gate)
    pub fn t_gate() -> ComplexMatrix {
        let phase = Complex64::from_polar(1.0, std::f64::consts::FRAC_PI_4);
        ComplexMatrix::new(
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                phase,
            ],
            2,
            2,
        )
    }

    /// CNOT gate (controlled-NOT)
    pub fn cnot() -> ComplexMatrix {
        ComplexMatrix::new(
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            4,
            4,
        )
    }

    /// SWAP gate
    pub fn swap() -> ComplexMatrix {
        ComplexMatrix::new(
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
            ],
            4,
            4,
        )
    }

    /// Rotation around X axis by angle theta
    pub fn rx(theta: f64) -> ComplexMatrix {
        let c = (theta / 2.0).cos();
        let s = (theta / 2.0).sin();
        ComplexMatrix::new(
            vec![
                Complex64::new(c, 0.0),
                Complex64::new(0.0, -s),
                Complex64::new(0.0, -s),
                Complex64::new(c, 0.0),
            ],
            2,
            2,
        )
    }

    /// Rotation around Y axis by angle theta
    pub fn ry(theta: f64) -> ComplexMatrix {
        let c = (theta / 2.0).cos();
        let s = (theta / 2.0).sin();
        ComplexMatrix::new(
            vec![
                Complex64::new(c, 0.0),
                Complex64::new(-s, 0.0),
                Complex64::new(s, 0.0),
                Complex64::new(c, 0.0),
            ],
            2,
            2,
        )
    }

    /// Rotation around Z axis by angle theta
    pub fn rz(theta: f64) -> ComplexMatrix {
        let phase_neg = Complex64::from_polar(1.0, -theta / 2.0);
        let phase_pos = Complex64::from_polar(1.0, theta / 2.0);
        ComplexMatrix::new(
            vec![
                phase_neg,
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                phase_pos,
            ],
            2,
            2,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complex_vector_basics() {
        let v = ComplexVector::new(vec![
            Complex64::new(1.0, 0.0),
            Complex64::new(0.0, 1.0),
        ]);
        assert_eq!(v.dim(), 2);
        assert!((v.norm_squared() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_inner_product() {
        let v1 = ComplexVector::new(vec![Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)]);
        let v2 = ComplexVector::new(vec![Complex64::new(0.0, 0.0), Complex64::new(1.0, 0.0)]);

        // Orthogonal vectors have zero inner product
        let inner = v1.inner(&v2);
        assert!((inner.norm()) < 1e-10);

        // Self inner product equals norm squared
        let self_inner = v1.inner(&v1);
        assert!((self_inner.re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_matrix_operations() {
        let identity = ComplexMatrix::identity(2);
        assert!(identity.is_square());
        assert!((identity.trace().re - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_hadamard_unitarity() {
        let h = gates::hadamard();
        assert!(h.is_unitary(1e-10));
    }

    #[test]
    fn test_pauli_matrices() {
        let x = gates::pauli_x();
        let y = gates::pauli_y();
        let z = gates::pauli_z();

        // X² = I
        let x2 = x.matmul(&x);
        assert!((x2.get(0, 0).re - 1.0).abs() < 1e-10);
        assert!((x2.get(1, 1).re - 1.0).abs() < 1e-10);

        // Y² = I
        let y2 = y.matmul(&y);
        assert!((y2.get(0, 0).re - 1.0).abs() < 1e-10);

        // Z² = I
        let z2 = z.matmul(&z);
        assert!((z2.get(0, 0).re - 1.0).abs() < 1e-10);

        // All are Hermitian
        assert!(x.is_hermitian(1e-10));
        assert!(y.is_hermitian(1e-10));
        assert!(z.is_hermitian(1e-10));
    }

    #[test]
    fn test_tensor_product() {
        let v1 = ComplexVector::new(vec![Complex64::new(1.0, 0.0), Complex64::new(0.0, 0.0)]);
        let v2 = ComplexVector::new(vec![Complex64::new(0.0, 0.0), Complex64::new(1.0, 0.0)]);

        let tensor = v1.tensor(&v2);
        assert_eq!(tensor.dim(), 4);
        // |0⟩ ⊗ |1⟩ = |01⟩ = [0, 1, 0, 0]
        assert!((tensor.data[1].re - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_partial_trace() {
        // Create a 4x4 matrix (2-qubit system)
        let mut m = ComplexMatrix::zeros(4, 4);
        // Set it to |00⟩⟨00| + |11⟩⟨11| (maximally entangled diagonal)
        m.set(0, 0, Complex64::new(0.5, 0.0));
        m.set(3, 3, Complex64::new(0.5, 0.0));

        // Partial trace over B should give maximally mixed state on A
        let reduced = m.partial_trace_b(2, 2);
        assert_eq!(reduced.rows, 2);
        assert!((reduced.get(0, 0).re - 0.5).abs() < 1e-10);
        assert!((reduced.get(1, 1).re - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_eigenvalues_2x2() {
        // Identity matrix has eigenvalues 1, 1
        let identity = ComplexMatrix::identity(2);
        let eigenvalues = identity.eigenvalues(100, 1e-10);
        assert_eq!(eigenvalues.len(), 2);
        for ev in &eigenvalues {
            assert!((ev.re - 1.0).abs() < 1e-5);
        }

        // Pauli Z has eigenvalues +1, -1
        let z = gates::pauli_z();
        let z_eigenvalues = z.eigenvalues(100, 1e-10);
        assert_eq!(z_eigenvalues.len(), 2);
    }
}
