//! Ultra-optimized CSR matrix implementation for beating Python benchmarks
//!
//! This module implements a high-performance CSR matrix specifically designed
//! to outperform NumPy/SciPy implementations through careful optimization.

use crate::types::Precision;
use crate::matrix::sparse::CSRStorage;
use alloc::vec::Vec;
use core::mem;

#[cfg(feature = "simd")]
use wide::f64x4;

/// Ultra-optimized CSR matrix with memory pooling and SIMD acceleration
pub struct OptimizedCSR {
    storage: CSRStorage,
    /// Memory pool for temporary vectors
    temp_pool: Vec<Vec<Precision>>,
    /// Performance counters
    matvec_count: usize,
}

impl OptimizedCSR {
    /// Create optimized CSR from triplets with memory pre-allocation
    pub fn from_triplets(
        triplets: Vec<(usize, usize, Precision)>,
        rows: usize,
        cols: usize,
    ) -> Result<Self, String> {
        let nnz = triplets.len();

        // Pre-allocate with exact capacity to avoid reallocations
        let mut storage = CSRStorage::with_capacity(rows, cols, nnz);

        // Sort triplets by (row, col) for optimal CSR construction
        let mut sorted_triplets = triplets;
        sorted_triplets.sort_by(|a, b| {
            a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1))
        });

        // Build CSR structure efficiently
        let mut current_row = 0;
        for (row, col, val) in sorted_triplets.iter() {
            // Update row pointers
            while current_row <= *row {
                storage.row_ptr[current_row] = storage.values.len() as u32;
                current_row += 1;
            }

            storage.values.push(*val);
            storage.col_indices.push(*col as u32);
        }

        // Fill remaining row pointers
        while current_row <= rows {
            storage.row_ptr[current_row] = storage.values.len() as u32;
            current_row += 1;
        }

        // Pre-allocate memory pool
        let temp_pool = vec![Vec::with_capacity(rows); 4];

        Ok(Self {
            storage,
            temp_pool,
            matvec_count: 0,
        })
    }

    /// Ultra-fast matrix-vector multiplication optimized to beat Python
    pub fn multiply_vector_optimized(&mut self, x: &[Precision], y: &mut [Precision]) {
        assert_eq!(x.len(), self.storage.cols);
        assert_eq!(y.len(), self.storage.rows);

        self.matvec_count += 1;

        #[cfg(feature = "simd")]
        {
            self.multiply_simd_optimized(x, y);
        }
        #[cfg(not(feature = "simd"))]
        {
            self.multiply_scalar_optimized(x, y);
        }
    }

    #[cfg(feature = "simd")]
    fn multiply_simd_optimized(&self, x: &[Precision], y: &mut [Precision]) {
        use std::arch::x86_64::*;

        // Zero output vector using SIMD
        let chunks = y.len() / 4;
        let remainder = y.len() % 4;

        unsafe {
            let zero = _mm256_setzero_pd();
            for chunk in 0..chunks {
                let idx = chunk * 4;
                _mm256_storeu_pd(y.as_mut_ptr().add(idx), zero);
            }

            // Handle remainder
            for i in (chunks * 4)..y.len() {
                y[i] = 0.0;
            }
        }

        // Process rows with optimized SIMD
        for row in 0..self.storage.rows {
            let start = self.storage.row_ptr[row] as usize;
            let end = self.storage.row_ptr[row + 1] as usize;
            let nnz = end - start;

            if nnz == 0 {
                continue;
            }

            let values = &self.storage.values[start..end];
            let indices = &self.storage.col_indices[start..end];

            if nnz >= 8 {
                // Use SIMD for larger rows
                let simd_chunks = nnz / 4;
                let mut sum = f64x4::splat(0.0);

                for chunk in 0..simd_chunks {
                    let idx = chunk * 4;

                    let vals = f64x4::new([
                        values[idx],
                        values[idx + 1],
                        values[idx + 2],
                        values[idx + 3],
                    ]);

                    let x_vals = f64x4::new([
                        x[indices[idx] as usize],
                        x[indices[idx + 1] as usize],
                        x[indices[idx + 2] as usize],
                        x[indices[idx + 3] as usize],
                    ]);

                    sum = sum + (vals * x_vals);
                }

                // Horizontal sum
                let sum_array = sum.to_array();
                y[row] = sum_array[0] + sum_array[1] + sum_array[2] + sum_array[3];

                // Handle remainder elements
                for i in (simd_chunks * 4)..nnz {
                    y[row] += values[i] * x[indices[i] as usize];
                }
            } else {
                // Scalar for small rows (avoid SIMD overhead)
                let mut sum = 0.0;
                for i in 0..nnz {
                    sum += values[i] * x[indices[i] as usize];
                }
                y[row] = sum;
            }
        }
    }

    #[cfg(not(feature = "simd"))]
    fn multiply_scalar_optimized(&self, x: &[Precision], y: &mut [Precision]) {
        // Optimized scalar implementation
        y.fill(0.0);

        for row in 0..self.storage.rows {
            let start = self.storage.row_ptr[row] as usize;
            let end = self.storage.row_ptr[row + 1] as usize;

            let mut sum = 0.0;
            let values = &self.storage.values[start..end];
            let indices = &self.storage.col_indices[start..end];

            // Unroll loop for better performance
            let chunks = (end - start) / 4;
            let remainder = (end - start) % 4;

            for chunk in 0..chunks {
                let idx = chunk * 4;
                sum += values[idx] * x[indices[idx] as usize]
                    + values[idx + 1] * x[indices[idx + 1] as usize]
                    + values[idx + 2] * x[indices[idx + 2] as usize]
                    + values[idx + 3] * x[indices[idx + 3] as usize];
            }

            // Handle remainder
            for i in (chunks * 4)..(chunks * 4 + remainder) {
                sum += values[i] * x[indices[i] as usize];
            }

            y[row] = sum;
        }
    }

    /// Get a temporary vector from the pool
    pub fn get_temp_vector(&mut self) -> Vec<Precision> {
        if let Some(mut vec) = self.temp_pool.pop() {
            vec.clear();
            vec.resize(self.storage.rows, 0.0);
            vec
        } else {
            vec![0.0; self.storage.rows]
        }
    }

    /// Return a temporary vector to the pool
    pub fn return_temp_vector(&mut self, vec: Vec<Precision>) {
        if self.temp_pool.len() < 8 {
            self.temp_pool.push(vec);
        }
    }

    /// Get matrix dimensions
    pub fn dimensions(&self) -> (usize, usize) {
        (self.storage.rows, self.storage.cols)
    }

    /// Get number of non-zeros
    pub fn nnz(&self) -> usize {
        self.storage.values.len()
    }

    /// Get performance statistics
    pub fn get_stats(&self) -> (usize, usize) {
        (self.matvec_count, self.temp_pool.len())
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn test_optimized_csr() {
        let triplets = vec![
            (0, 0, 4.0), (0, 1, 1.0),
            (1, 0, 2.0), (1, 1, 3.0),
        ];

        let mut matrix = OptimizedCSR::from_triplets(triplets, 2, 2).unwrap();
        let x = vec![1.0, 2.0];
        let mut y = vec![0.0; 2];

        matrix.multiply_vector_optimized(&x, &mut y);
        assert_eq!(y, vec![6.0, 8.0]);
    }
}