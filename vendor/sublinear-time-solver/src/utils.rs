//! Utility functions and helpers for the sublinear solver.
//!
//! This module provides common mathematical operations, memory management
//! utilities, and performance optimization helpers.

use crate::types::Precision;
use alloc::vec::Vec;

/// Mathematical utility functions.
pub mod math {
    use super::*;

    /// Compute dot product of two vectors.
    pub fn dot_product(a: &[Precision], b: &[Precision]) -> Precision {
        a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum()
    }

    /// Compute vector addition: c = a + b
    pub fn vector_add(a: &[Precision], b: &[Precision], c: &mut [Precision]) {
        for ((c_i, &a_i), &b_i) in c.iter_mut().zip(a.iter()).zip(b.iter()) {
            *c_i = a_i + b_i;
        }
    }

    /// Compute vector subtraction: c = a - b
    pub fn vector_sub(a: &[Precision], b: &[Precision], c: &mut [Precision]) {
        for ((c_i, &a_i), &b_i) in c.iter_mut().zip(a.iter()).zip(b.iter()) {
            *c_i = a_i - b_i;
        }
    }

    /// Scale vector by scalar: b = alpha * a
    pub fn vector_scale(alpha: Precision, a: &[Precision], b: &mut [Precision]) {
        for (b_i, &a_i) in b.iter_mut().zip(a.iter()) {
            *b_i = alpha * a_i;
        }
    }

    /// Compute AXPY operation: y = alpha * x + y
    pub fn axpy(alpha: Precision, x: &[Precision], y: &mut [Precision]) {
        for (y_i, &x_i) in y.iter_mut().zip(x.iter()) {
            *y_i += alpha * x_i;
        }
    }
}

/// Memory management utilities.
pub mod memory {
    use super::*;

    /// Simple memory pool for vector allocation.
    pub struct VectorPool {
        pools: Vec<Vec<Vec<Precision>>>,
        max_size: usize,
    }

    impl VectorPool {
        /// Create a new vector pool.
        pub fn new(max_size: usize) -> Self {
            Self {
                pools: Vec::new(),
                max_size,
            }
        }

        /// Get a vector from the pool or allocate a new one.
        pub fn get_vector(&mut self, size: usize) -> Vec<Precision> {
            if size <= self.max_size {
                // Try to find a suitable pool
                while self.pools.len() <= size {
                    self.pools.push(Vec::new());
                }

                if let Some(vec) = self.pools[size].pop() {
                    return vec;
                }
            }

            vec![0.0; size]
        }

        /// Return a vector to the pool.
        pub fn return_vector(&mut self, mut vec: Vec<Precision>) {
            let size = vec.len();
            if size <= self.max_size && vec.capacity() == size {
                vec.clear();
                vec.resize(size, 0.0);

                while self.pools.len() <= size {
                    self.pools.push(Vec::new());
                }

                self.pools[size].push(vec);
            }
        }
    }
}

/// Performance optimization utilities.
pub mod perf {
    use super::*;

    /// Check if SIMD operations are available.
    pub fn has_simd() -> bool {
        #[cfg(feature = "simd")]
        {
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            {
                return is_x86_feature_detected!("avx2");
            }
            #[cfg(target_arch = "aarch64")]
            {
                return std::arch::is_aarch64_feature_detected!("neon");
            }
        }
        false
    }

    /// Prefetch memory for better cache performance.
    #[inline(always)]
    pub fn prefetch_read<T>(ptr: *const T) {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            unsafe {
                core::arch::x86_64::_mm_prefetch(ptr as *const i8, core::arch::x86_64::_MM_HINT_T0);
            }
        }
    }
}

/// Numerical analysis utilities.
pub mod numerical {
    use super::*;

    /// Machine epsilon for the precision type.
    pub const MACHINE_EPSILON: Precision = 2.220446049250313e-16;

    /// Check if a number is effectively zero.
    pub fn is_zero(x: Precision) -> bool {
        x.abs() < 10.0 * MACHINE_EPSILON
    }

    /// Check if two numbers are approximately equal.
    pub fn approx_equal(a: Precision, b: Precision, tol: Precision) -> bool {
        (a - b).abs() <= tol * (1.0 + a.abs().max(b.abs()))
    }

    /// Compute condition number estimate using power iteration.
    pub fn condition_number_estimate(matrix_op: impl Fn(&[Precision], &mut [Precision]), n: usize, max_iter: usize) -> Precision {
        let mut x = vec![1.0 / (n as Precision).sqrt(); n];
        let mut y = vec![0.0; n];

        let mut lambda_max = 0.0;

        for _ in 0..max_iter {
            // y = A * x
            matrix_op(&x, &mut y);

            // Compute eigenvalue estimate
            lambda_max = math::dot_product(&x, &y);

            // Normalize: x = y / ||y||
            let norm = (math::dot_product(&y, &y)).sqrt();
            if norm > 0.0 {
                for (x_i, &y_i) in x.iter_mut().zip(y.iter()) {
                    *x_i = y_i / norm;
                }
            }
        }

        // This is a simplified estimate - real condition number requires min eigenvalue too
        lambda_max
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn test_vector_operations() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let mut c = vec![0.0; 3];

        math::vector_add(&a, &b, &mut c);
        assert_eq!(c, vec![5.0, 7.0, 9.0]);

        math::vector_sub(&b, &a, &mut c);
        assert_eq!(c, vec![3.0, 3.0, 3.0]);

        let dot = math::dot_product(&a, &b);
        assert_eq!(dot, 32.0); // 1*4 + 2*5 + 3*6
    }

    #[test]
    fn test_vector_pool() {
        let mut pool = memory::VectorPool::new(10);

        let vec1 = pool.get_vector(5);
        assert_eq!(vec1.len(), 5);

        pool.return_vector(vec1);

        let vec2 = pool.get_vector(5);
        assert_eq!(vec2.len(), 5);
        // Should be the same allocation (though we can't test that directly)
    }

    #[test]
    fn test_numerical_utilities() {
        assert!(numerical::is_zero(1e-17));
        assert!(!numerical::is_zero(1e-10));

        assert!(numerical::approx_equal(1.0, 1.0 + 1e-12, 1e-10));
        assert!(!numerical::approx_equal(1.0, 1.1, 1e-10));
    }
}