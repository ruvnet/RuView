//! Core mathematical structures for FTL information system

use ndarray::{Array2, Array1, ArrayView2, ArrayView1};
use sprs::{CsMat, TriMat};
use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, Sub};

/// Dense matrix representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Matrix {
    data: Array2<f64>,
}

impl Matrix {
    /// Create a new matrix from 2D array
    pub fn new(data: Array2<f64>) -> Self {
        Self { data }
    }

    /// Create a random matrix for testing
    pub fn random(rows: usize, cols: usize) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let data = Array2::from_shape_fn((rows, cols), |_| rng.gen_range(-1.0..1.0));
        Self { data }
    }

    /// Create a diagonally dominant matrix (guaranteed solvable)
    pub fn diagonally_dominant(size: usize, dominance_factor: f64) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut data = Array2::zeros((size, size));

        for i in 0..size {
            let mut row_sum = 0.0;
            for j in 0..size {
                if i != j {
                    let val = rng.gen_range(-1.0..1.0);
                    data[[i, j]] = val;
                    row_sum += val.abs();
                }
            }
            // Make diagonal dominant
            data[[i, i]] = row_sum * dominance_factor;
        }

        Self { data }
    }

    /// Get matrix dimensions
    pub fn shape(&self) -> (usize, usize) {
        let shape = self.data.shape();
        (shape[0], shape[1])
    }

    /// Get matrix as array view
    pub fn view(&self) -> ArrayView2<f64> {
        self.data.view()
    }

    /// Check if matrix is square
    pub fn is_square(&self) -> bool {
        let (rows, cols) = self.shape();
        rows == cols
    }

    /// Compute spectral radius (largest eigenvalue magnitude)
    pub fn spectral_radius(&self) -> f64 {
        // Simplified power iteration method
        if !self.is_square() {
            return 0.0;
        }

        let n = self.shape().0;
        let mut v = Vector::ones(n);
        let max_iter = 100;

        for _ in 0..max_iter {
            let new_v = self.multiply_vector(&v);
            let norm = new_v.norm();
            if norm > 0.0 {
                v = new_v.scale(1.0 / norm);
            }
        }

        let mv = self.multiply_vector(&v);
        mv.dot(&v) / v.dot(&v)
    }

    /// Multiply matrix by vector
    pub fn multiply_vector(&self, v: &Vector) -> Vector {
        let result = self.data.dot(&v.data);
        Vector::new(result)
    }

    /// Convert to sparse format
    pub fn to_sparse(&self) -> SparseMatrix {
        let (rows, cols) = self.shape();
        let mut tri = TriMat::new((rows, cols));

        for i in 0..rows {
            for j in 0..cols {
                let val = self.data[[i, j]];
                if val.abs() > 1e-10 {
                    tri.add_triplet(i, j, val);
                }
            }
        }

        SparseMatrix {
            data: tri.to_csr(),
        }
    }
}

/// Dense vector representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vector {
    data: Array1<f64>,
}

impl Vector {
    /// Create a new vector
    pub fn new(data: Array1<f64>) -> Self {
        Self { data }
    }

    /// Create a vector of ones
    pub fn ones(size: usize) -> Self {
        Self {
            data: Array1::ones(size),
        }
    }

    /// Create a vector of zeros
    pub fn zeros(size: usize) -> Self {
        Self {
            data: Array1::zeros(size),
        }
    }

    /// Create a random vector
    pub fn random(size: usize) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let data = Array1::from_shape_fn(size, |_| rng.gen_range(-1.0..1.0));
        Self { data }
    }

    /// Get vector length
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if vector is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Compute L2 norm
    pub fn norm(&self) -> f64 {
        self.data.dot(&self.data).sqrt()
    }

    /// Dot product with another vector
    pub fn dot(&self, other: &Vector) -> f64 {
        self.data.dot(&other.data)
    }

    /// Scale vector by scalar
    pub fn scale(&self, scalar: f64) -> Self {
        Self {
            data: &self.data * scalar,
        }
    }

    /// Get as array view
    pub fn view(&self) -> ArrayView1<f64> {
        self.data.view()
    }

    /// Add another vector
    pub fn add(&self, other: &Vector) -> Self {
        Self {
            data: &self.data + &other.data,
        }
    }

    /// Subtract another vector
    pub fn sub(&self, other: &Vector) -> Self {
        Self {
            data: &self.data - &other.data,
        }
    }
}

/// Sparse matrix representation for efficient large-scale computation
#[derive(Debug, Clone)]
pub struct SparseMatrix {
    data: CsMat<f64>,
}

impl SparseMatrix {
    /// Create from CSR data
    pub fn from_csr(data: CsMat<f64>) -> Self {
        Self { data }
    }

    /// Get number of non-zero elements
    pub fn nnz(&self) -> usize {
        self.data.nnz()
    }

    /// Get shape
    pub fn shape(&self) -> (usize, usize) {
        self.data.shape()
    }

    /// Multiply by vector
    pub fn multiply_vector(&self, v: &Vector) -> Vector {
        let result = &self.data * v.view();
        Vector::new(result.to_owned())
    }

    /// Get sparsity (percentage of zeros)
    pub fn sparsity(&self) -> f64 {
        let (rows, cols) = self.shape();
        let total = rows * cols;
        if total == 0 {
            return 1.0;
        }
        1.0 - (self.nnz() as f64 / total as f64)
    }
}

/// Complexity class for algorithm analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Complexity {
    /// O(1) - Constant time
    Constant,
    /// O(log n) - Logarithmic (our target)
    Logarithmic,
    /// O(n) - Linear
    Linear,
    /// O(n log n) - Linearithmic
    Linearithmic,
    /// O(n²) - Quadratic
    Quadratic,
    /// O(n³) - Cubic (traditional matrix operations)
    Cubic,
}

impl Complexity {
    /// Estimate time for given input size (nanoseconds)
    pub fn estimate_time_ns(&self, n: usize) -> u64 {
        const BASE_TIME: u64 = 10; // 10ns base operation
        match self {
            Complexity::Constant => BASE_TIME,
            Complexity::Logarithmic => BASE_TIME * (n as f64).log2() as u64,
            Complexity::Linear => BASE_TIME * n as u64,
            Complexity::Linearithmic => BASE_TIME * n as u64 * (n as f64).log2() as u64,
            Complexity::Quadratic => BASE_TIME * (n * n) as u64,
            Complexity::Cubic => BASE_TIME * (n * n * n) as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_creation() {
        let m = Matrix::diagonally_dominant(10, 2.0);
        assert_eq!(m.shape(), (10, 10));
        assert!(m.is_square());
    }

    #[test]
    fn test_vector_operations() {
        let v1 = Vector::ones(5);
        let v2 = Vector::ones(5);
        let v3 = v1.add(&v2);
        assert_eq!(v3.data[0], 2.0);
    }

    #[test]
    fn test_complexity_estimation() {
        let n = 1000;
        let log_time = Complexity::Logarithmic.estimate_time_ns(n);
        let cubic_time = Complexity::Cubic.estimate_time_ns(n);
        assert!(log_time < cubic_time / 1000000);
    }
}