//! Ultra-fast solver optimized to beat Python benchmarks
//!
//! This module implements the critical optimizations needed to address the
//! MCP Dense performance issue that's 190x slower than Python baselines.

use crate::types::Precision;

/// Ultra-optimized CSR matrix for performance-critical operations
#[derive(Debug, Clone)]
pub struct FastCSRMatrix {
    pub values: Vec<Precision>,
    pub col_indices: Vec<u32>,
    pub row_ptr: Vec<u32>,
    pub rows: usize,
    pub cols: usize,
}

impl FastCSRMatrix {
    /// Create from triplets with performance optimizations
    pub fn from_triplets(
        triplets: Vec<(usize, usize, Precision)>,
        rows: usize,
        cols: usize,
    ) -> Self {
        // Pre-allocate with exact capacity
        let mut sorted_triplets = triplets;
        sorted_triplets.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

        let nnz = sorted_triplets.len();
        let mut values = Vec::with_capacity(nnz);
        let mut col_indices = Vec::with_capacity(nnz);
        let mut row_ptr = vec![0u32; rows + 1];

        let mut current_row = 0;
        for (row, col, val) in sorted_triplets {
            // Fill row pointers
            while current_row <= row {
                row_ptr[current_row] = values.len() as u32;
                current_row += 1;
            }

            values.push(val);
            col_indices.push(col as u32);
        }

        // Fill remaining row pointers
        while current_row <= rows {
            row_ptr[current_row] = values.len() as u32;
            current_row += 1;
        }

        Self {
            values,
            col_indices,
            row_ptr,
            rows,
            cols,
        }
    }

    /// Ultra-fast matrix-vector multiplication optimized for performance
    pub fn multiply_vector_fast(&self, x: &[Precision], y: &mut [Precision]) {
        y.fill(0.0);

        // Process rows with manual loop unrolling for better performance
        for row in 0..self.rows {
            let start = self.row_ptr[row] as usize;
            let end = self.row_ptr[row + 1] as usize;

            if start >= end {
                continue;
            }

            let nnz = end - start;
            let values = &self.values[start..end];
            let indices = &self.col_indices[start..end];

            // Unroll for small rows
            if nnz <= 4 {
                let mut sum = 0.0;
                for i in 0..nnz {
                    sum += values[i] * x[indices[i] as usize];
                }
                y[row] = sum;
            } else {
                // Process in chunks of 4 for larger rows
                let chunks = nnz / 4;
                let remainder = nnz % 4;
                let mut sum = 0.0;

                // Process 4 elements at a time
                for chunk in 0..chunks {
                    let base = chunk * 4;
                    sum += values[base] * x[indices[base] as usize]
                        + values[base + 1] * x[indices[base + 1] as usize]
                        + values[base + 2] * x[indices[base + 2] as usize]
                        + values[base + 3] * x[indices[base + 3] as usize];
                }

                // Handle remainder
                for i in (chunks * 4)..(chunks * 4 + remainder) {
                    sum += values[i] * x[indices[i] as usize];
                }

                y[row] = sum;
            }
        }
    }
}

/// Fast conjugate gradient solver optimized for sparse matrices
pub struct FastConjugateGradient {
    pub max_iterations: usize,
    pub tolerance: Precision,
}

impl FastConjugateGradient {
    pub fn new(max_iterations: usize, tolerance: Precision) -> Self {
        Self {
            max_iterations,
            tolerance,
        }
    }

    /// Solve Ax = b with optimized conjugate gradient
    pub fn solve(&self, matrix: &FastCSRMatrix, b: &[Precision]) -> Vec<Precision> {
        let n = matrix.rows;
        assert_eq!(b.len(), n);
        assert_eq!(matrix.rows, matrix.cols);

        // Pre-allocate all vectors
        let mut x = vec![0.0; n];
        let mut r = vec![0.0; n];
        let mut p = vec![0.0; n];
        let mut ap = vec![0.0; n];

        // r = b - A*x (initially r = b since x = 0)
        r.copy_from_slice(b);
        p.copy_from_slice(b);

        let mut rsold = Self::dot_product_fast(&r, &r);
        let tolerance_sq = self.tolerance * self.tolerance;

        for _iteration in 0..self.max_iterations {
            if rsold <= tolerance_sq {
                break;
            }

            // ap = A * p
            matrix.multiply_vector_fast(&p, &mut ap);

            // alpha = rsold / (p^T * ap)
            let pap = Self::dot_product_fast(&p, &ap);
            if pap.abs() < 1e-16 {
                break;
            }

            let alpha = rsold / pap;

            // x = x + alpha * p
            Self::axpy_fast(alpha, &p, &mut x);

            // r = r - alpha * ap
            Self::axpy_fast(-alpha, &ap, &mut r);

            let rsnew = Self::dot_product_fast(&r, &r);
            let beta = rsnew / rsold;

            // p = r + beta * p
            for i in 0..n {
                p[i] = r[i] + beta * p[i];
            }

            rsold = rsnew;
        }

        x
    }

    /// Fast dot product with manual unrolling
    #[inline]
    fn dot_product_fast(x: &[Precision], y: &[Precision]) -> Precision {
        let n = x.len();
        let chunks = n / 4;
        let remainder = n % 4;
        let mut sum = 0.0;

        // Process 4 elements at a time
        for chunk in 0..chunks {
            let base = chunk * 4;
            sum += x[base] * y[base]
                + x[base + 1] * y[base + 1]
                + x[base + 2] * y[base + 2]
                + x[base + 3] * y[base + 3];
        }

        // Handle remainder
        for i in (chunks * 4)..(chunks * 4 + remainder) {
            sum += x[i] * y[i];
        }

        sum
    }

    /// Fast AXPY operation: y = alpha * x + y
    #[inline]
    fn axpy_fast(alpha: Precision, x: &[Precision], y: &mut [Precision]) {
        let n = x.len();
        let chunks = n / 4;
        let remainder = n % 4;

        // Process 4 elements at a time
        for chunk in 0..chunks {
            let base = chunk * 4;
            y[base] += alpha * x[base];
            y[base + 1] += alpha * x[base + 1];
            y[base + 2] += alpha * x[base + 2];
            y[base + 3] += alpha * x[base + 3];
        }

        // Handle remainder
        for i in (chunks * 4)..(chunks * 4 + remainder) {
            y[i] += alpha * x[i];
        }
    }
}

/// Memory-efficient buffer pool for reusing vectors
pub struct VectorPool {
    buffers: Vec<Vec<Precision>>,
    size: usize,
}

impl VectorPool {
    pub fn new(size: usize, capacity: usize) -> Self {
        let buffers = (0..capacity)
            .map(|_| vec![0.0; size])
            .collect();

        Self { buffers, size }
    }

    pub fn get_buffer(&mut self) -> Vec<Precision> {
        self.buffers.pop().unwrap_or_else(|| vec![0.0; self.size])
    }

    pub fn return_buffer(&mut self, mut buffer: Vec<Precision>) {
        if buffer.len() == self.size && self.buffers.len() < 8 {
            buffer.fill(0.0);
            self.buffers.push(buffer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_csr_matrix() {
        let triplets = vec![
            (0, 0, 4.0), (0, 1, 1.0),
            (1, 0, 2.0), (1, 1, 3.0),
        ];

        let matrix = FastCSRMatrix::from_triplets(triplets, 2, 2);
        let x = vec![1.0, 2.0];
        let mut y = vec![0.0; 2];

        matrix.multiply_vector_fast(&x, &mut y);
        assert_eq!(y, vec![6.0, 8.0]); // [4*1+1*2, 2*1+3*2]
    }

    #[test]
    fn test_fast_conjugate_gradient() {
        let triplets = vec![
            (0, 0, 4.0), (0, 1, 1.0),
            (1, 0, 1.0), (1, 1, 3.0),
        ];

        let matrix = FastCSRMatrix::from_triplets(triplets, 2, 2);
        let b = vec![1.0, 2.0];

        let solver = FastConjugateGradient::new(1000, 1e-10);
        let solution = solver.solve(&matrix, &b);

        // Verify solution by substituting back
        let mut result = vec![0.0; 2];
        matrix.multiply_vector_fast(&solution, &mut result);

        let error = ((result[0] - b[0]).powi(2) + (result[1] - b[1]).powi(2)).sqrt();
        assert!(error < 1e-8);
    }
}