//! SIMD-accelerated linear algebra operations for high-performance computing.
//!
//! This module provides vectorized implementations of core matrix and vector
//! operations using SIMD intrinsics for maximum performance on modern CPUs.

use crate::types::Precision;
use alloc::vec::Vec;

#[cfg(feature = "simd")]
use wide::f64x4;

#[cfg(all(feature = "std", feature = "rayon"))]
use rayon::prelude::*;

/// SIMD-accelerated matrix-vector multiplication: y = A * x
///
/// This function uses SIMD intrinsics to perform matrix-vector multiplication
/// with optimal memory access patterns and vectorization.
#[cfg(feature = "simd")]
pub fn matrix_vector_multiply_simd(
    values: &[Precision],
    col_indices: &[u32],
    row_ptr: &[u32],
    x: &[Precision],
    y: &mut [Precision],
) {
    y.fill(0.0);

    for row in 0..y.len() {
        let start = row_ptr[row] as usize;
        let end = row_ptr[row + 1] as usize;

        if end <= start {
            continue;
        }

        let row_values = &values[start..end];
        let row_indices = &col_indices[start..end];
        let nnz = row_values.len();

        if nnz >= 8 {
            // Process in chunks of 4 for AVX2/SIMD128
            let simd_chunks = nnz / 4;
            let mut sum = f64x4::splat(0.0);

            for chunk in 0..simd_chunks {
                let idx = chunk * 4;

                // Load 4 matrix values
                let vals = f64x4::new([
                    row_values[idx],
                    row_values[idx + 1],
                    row_values[idx + 2],
                    row_values[idx + 3],
                ]);

                // Load corresponding x values (gather operation)
                let x_vals = f64x4::new([
                    x[row_indices[idx] as usize],
                    x[row_indices[idx + 1] as usize],
                    x[row_indices[idx + 2] as usize],
                    x[row_indices[idx + 3] as usize],
                ]);

                // Multiply and accumulate
                sum = sum + (vals * x_vals);
            }

            // Sum the SIMD register horizontally
            let sum_array = sum.to_array();
            y[row] = sum_array[0] + sum_array[1] + sum_array[2] + sum_array[3];

            // Handle remaining elements
            for i in (simd_chunks * 4)..nnz {
                let col = row_indices[i] as usize;
                y[row] += row_values[i] * x[col];
            }
        } else {
            // For small rows, use scalar code (avoid SIMD overhead)
            let mut sum = 0.0;
            for i in 0..nnz {
                let col = row_indices[i] as usize;
                sum += row_values[i] * x[col];
            }
            y[row] = sum;
        }
    }
}

/// Fallback implementation for when SIMD is not available
#[cfg(not(feature = "simd"))]
pub fn matrix_vector_multiply_simd(
    values: &[Precision],
    col_indices: &[u32],
    row_ptr: &[u32],
    x: &[Precision],
    y: &mut [Precision],
) {
    y.fill(0.0);

    for row in 0..y.len() {
        let start = row_ptr[row] as usize;
        let end = row_ptr[row + 1] as usize;

        let mut sum = 0.0;
        for i in start..end {
            let col = col_indices[i] as usize;
            sum += values[i] * x[col];
        }
        y[row] = sum;
    }
}

/// SIMD-accelerated dot product: result = x^T * y
#[cfg(feature = "simd")]
pub fn dot_product_simd(x: &[Precision], y: &[Precision]) -> Precision {
    assert_eq!(x.len(), y.len());

    let n = x.len();
    let simd_chunks = n / 4;
    let mut sum = f64x4::splat(0.0);

    // Process in chunks of 4
    for chunk in 0..simd_chunks {
        let idx = chunk * 4;

        let x_vals = f64x4::new([
            x[idx], x[idx + 1], x[idx + 2], x[idx + 3]
        ]);
        let y_vals = f64x4::new([
            y[idx], y[idx + 1], y[idx + 2], y[idx + 3]
        ]);

        sum = sum + (x_vals * y_vals);
    }

    // Sum the SIMD register
    let sum_array = sum.to_array();
    let mut result = sum_array[0] + sum_array[1] + sum_array[2] + sum_array[3];

    // Handle remaining elements
    for i in (simd_chunks * 4)..n {
        result += x[i] * y[i];
    }

    result
}

/// Fallback dot product implementation
#[cfg(not(feature = "simd"))]
pub fn dot_product_simd(x: &[Precision], y: &[Precision]) -> Precision {
    assert_eq!(x.len(), y.len());
    x.iter().zip(y.iter()).map(|(a, b)| a * b).sum()
}

/// SIMD-accelerated AXPY operation: y = alpha * x + y
#[cfg(feature = "simd")]
pub fn axpy_simd(alpha: Precision, x: &[Precision], y: &mut [Precision]) {
    assert_eq!(x.len(), y.len());

    let n = x.len();
    let simd_chunks = n / 4;
    let alpha_vec = f64x4::splat(alpha);

    // Process in chunks of 4
    for chunk in 0..simd_chunks {
        let idx = chunk * 4;

        let x_vals = f64x4::new([
            x[idx], x[idx + 1], x[idx + 2], x[idx + 3]
        ]);
        let y_vals = f64x4::new([
            y[idx], y[idx + 1], y[idx + 2], y[idx + 3]
        ]);

        let result = (alpha_vec * x_vals) + y_vals;
        let result_array = result.to_array();

        y[idx] = result_array[0];
        y[idx + 1] = result_array[1];
        y[idx + 2] = result_array[2];
        y[idx + 3] = result_array[3];
    }

    // Handle remaining elements
    for i in (simd_chunks * 4)..n {
        y[i] += alpha * x[i];
    }
}

/// Fallback AXPY implementation
#[cfg(not(feature = "simd"))]
pub fn axpy_simd(alpha: Precision, x: &[Precision], y: &mut [Precision]) {
    assert_eq!(x.len(), y.len());
    for (y_val, &x_val) in y.iter_mut().zip(x.iter()) {
        *y_val += alpha * x_val;
    }
}

/// Parallel matrix-vector multiplication using Rayon for very large matrices
#[cfg(all(feature = "std", feature = "rayon"))]
pub fn parallel_matrix_vector_multiply(
    values: &[Precision],
    col_indices: &[u32],
    row_ptr: &[u32],
    x: &[Precision],
    y: &mut [Precision],
    num_threads: Option<usize>,
) {
    y.fill(0.0);

    let num_threads = num_threads.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(1)
    });

    let rows = y.len();
    let chunk_size = (rows + num_threads - 1) / num_threads;

    y.par_chunks_mut(chunk_size)
        .enumerate()
        .for_each(|(chunk_idx, y_chunk)| {
            let start_row = chunk_idx * chunk_size;
            let end_row = (start_row + y_chunk.len()).min(rows);

            for (local_idx, global_row) in (start_row..end_row).enumerate() {
                let start = row_ptr[global_row] as usize;
                let end = row_ptr[global_row + 1] as usize;

                let mut sum = 0.0;
                for i in start..end {
                    let col = col_indices[i] as usize;
                    sum += values[i] * x[col];
                }
                y_chunk[local_idx] = sum;
            }
        });
}

/// Fallback parallel implementation
#[cfg(not(all(feature = "std", feature = "rayon")))]
pub fn parallel_matrix_vector_multiply(
    values: &[Precision],
    col_indices: &[u32],
    row_ptr: &[u32],
    x: &[Precision],
    y: &mut [Precision],
    _num_threads: Option<usize>,
) {
    matrix_vector_multiply_simd(values, col_indices, row_ptr, x, y);
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn test_simd_matrix_vector_multiply() {
        let values = vec![2.0, 1.0, 1.0, 3.0];
        let col_indices = vec![0, 1, 0, 1];
        let row_ptr = vec![0, 2, 4];
        let x = vec![1.0, 2.0];
        let mut y = vec![0.0; 2];

        matrix_vector_multiply_simd(&values, &col_indices, &row_ptr, &x, &mut y);
        assert_eq!(y, vec![4.0, 7.0]);
    }

    #[test]
    fn test_simd_dot_product() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 3.0, 4.0, 5.0, 6.0];
        let result = dot_product_simd(&x, &y);
        assert_eq!(result, 70.0); // 1*2 + 2*3 + 3*4 + 4*5 + 5*6
    }

    #[test]
    fn test_simd_axpy() {
        let alpha = 2.0;
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let mut y = vec![1.0, 1.0, 1.0, 1.0];

        axpy_simd(alpha, &x, &mut y);
        assert_eq!(y, vec![3.0, 5.0, 7.0, 9.0]);
    }
}