//! Core types and utilities for the hybrid solver implementation
//!
//! This module provides compatibility types that bridge between the new hybrid solver
//! implementation and the existing codebase structure.

use std::collections::HashMap;
pub use crate::error::{SolverError as SublinearError, Result};
pub use crate::types::{Precision as f64};

/// Type alias for vector operations
pub type Vector = Vec<f64>;

/// Sparse matrix implementation compatible with existing codebase
#[derive(Debug, Clone)]
pub struct SparseMatrix {
    rows: usize,
    cols: usize,
    data: HashMap<usize, HashMap<usize, f64>>,
}

impl SparseMatrix {
    /// Create a new sparse matrix with given dimensions
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: HashMap::new(),
        }
    }

    /// Insert a value at (row, col)
    pub fn insert(&mut self, row: usize, col: usize, value: f64) {
        if value != 0.0 {
            self.data.entry(row).or_default().insert(col, value);
        }
    }

    /// Get the number of rows
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Get the number of columns
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Get a specific element
    pub fn get(&self, row: usize, col: usize) -> Option<f64> {
        self.data.get(&row)?.get(&col).copied()
    }

    /// Get a row as a reference to the internal map
    pub fn get_row(&self, row: usize) -> &HashMap<usize, f64> {
        static EMPTY: HashMap<usize, f64> = HashMap::new();
        self.data.get(&row).unwrap_or(&EMPTY)
    }

    /// Create transpose of the matrix
    pub fn transpose(&self) -> SparseMatrix {
        let mut transposed = SparseMatrix::new(self.cols, self.rows);

        for (&row, row_data) in &self.data {
            for (&col, &value) in row_data {
                transposed.insert(col, row, value);
            }
        }

        transposed
    }

    /// Create from triplets (row, col, value)
    pub fn from_triplets(triplets: Vec<(usize, usize, f64)>, rows: usize, cols: usize) -> Self {
        let mut matrix = Self::new(rows, cols);
        for (row, col, value) in triplets {
            matrix.insert(row, col, value);
        }
        matrix
    }
}

/// Create compatibility module for existing algorithm traits
pub mod algorithms {
    use super::*;

    /// Precision levels for convergence
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum Precision {
        Low,
        Medium,
        High,
    }

    /// Convergence metrics for algorithms
    #[derive(Debug, Clone)]
    pub struct ConvergenceMetrics {
        pub iterations: usize,
        pub residual: f64,
        pub convergence_rate: f64,
        pub precision: Precision,
    }

    /// Algorithm trait for solving linear systems
    pub trait Algorithm {
        fn solve(&mut self, matrix: &SparseMatrix, target: &Vector) -> Result<Vector>;
        fn get_metrics(&self) -> ConvergenceMetrics;
        fn update_config(&mut self, params: HashMap<String, f64>);
    }
}

/// Test utilities module
#[cfg(test)]
pub mod test_utils {
    use super::*;

    /// Compute residual ||Ax - b||
    pub fn compute_residual(a: &SparseMatrix, x: &Vector, b: &Vector) -> f64 {
        let mut residual = vec![0.0; b.len()];

        // Compute Ax
        for i in 0..a.rows() {
            let row = a.get_row(i);
            for (&j, &value) in row {
                residual[i] += value * x[j];
            }
            residual[i] -= b[i];
        }

        // Compute L2 norm
        residual.iter().map(|r| r.powi(2)).sum::<f64>().sqrt()
    }

    /// Create a simple test matrix
    pub fn create_test_matrix(n: usize) -> SparseMatrix {
        let mut matrix = SparseMatrix::new(n, n);
        for i in 0..n {
            matrix.insert(i, i, 2.0);
            if i > 0 {
                matrix.insert(i, i-1, -0.5);
            }
            if i < n-1 {
                matrix.insert(i, i+1, -0.5);
            }
        }
        matrix
    }
}