//! Cache-optimized data structures using Structure-of-Arrays (SoA) layout
//!
//! This module provides cache-friendly layouts for vector storage to minimize
//! cache misses and improve memory access patterns.

use std::alloc::{alloc, dealloc, Layout};
use std::ptr;

/// Cache line size (typically 64 bytes on modern CPUs)
const CACHE_LINE_SIZE: usize = 64;

/// Structure-of-Arrays layout for vectors
///
/// Instead of storing vectors as Vec<Vec<f32>>, we store all components
/// separately to improve cache locality during SIMD operations.
#[repr(align(64))] // Align to cache line boundary
pub struct SoAVectorStorage {
    /// Number of vectors
    count: usize,
    /// Dimensions per vector
    dimensions: usize,
    /// Capacity (allocated vectors)
    capacity: usize,
    /// Storage for each dimension separately
    /// Layout: [dim0_vec0, dim0_vec1, ..., dim0_vecN, dim1_vec0, ...]
    data: *mut f32,
}

impl SoAVectorStorage {
    /// Maximum allowed dimensions to prevent overflow
    const MAX_DIMENSIONS: usize = 65536;
    /// Maximum allowed capacity to prevent overflow
    const MAX_CAPACITY: usize = 1 << 24; // ~16M vectors

    /// Create a new SoA vector storage
    ///
    /// # Panics
    /// Panics if dimensions or capacity exceed safe limits or would cause overflow.
    pub fn new(dimensions: usize, initial_capacity: usize) -> Self {
        // Security: Validate inputs to prevent integer overflow
        assert!(
            dimensions > 0 && dimensions <= Self::MAX_DIMENSIONS,
            "dimensions must be between 1 and {}",
            Self::MAX_DIMENSIONS
        );
        assert!(
            initial_capacity <= Self::MAX_CAPACITY,
            "initial_capacity exceeds maximum of {}",
            Self::MAX_CAPACITY
        );

        let capacity = initial_capacity.next_power_of_two();

        // Security: Use checked arithmetic to prevent overflow
        let total_elements = dimensions
            .checked_mul(capacity)
            .expect("dimensions * capacity overflow");
        let total_bytes = total_elements
            .checked_mul(std::mem::size_of::<f32>())
            .expect("total size overflow");

        let layout =
            Layout::from_size_align(total_bytes, CACHE_LINE_SIZE).expect("invalid memory layout");

        let data = unsafe { alloc(layout) as *mut f32 };

        // Zero initialize
        unsafe {
            ptr::write_bytes(data, 0, total_elements);
        }

        Self {
            count: 0,
            dimensions,
            capacity,
            data,
        }
    }

    /// Add a vector to the storage
    pub fn push(&mut self, vector: &[f32]) {
        assert_eq!(vector.len(), self.dimensions);

        if self.count >= self.capacity {
            self.grow();
        }

        // Store each dimension separately
        for (dim_idx, &value) in vector.iter().enumerate() {
            let offset = dim_idx * self.capacity + self.count;
            unsafe {
                *self.data.add(offset) = value;
            }
        }

        self.count += 1;
    }

    /// Get a vector by index (copies to output buffer)
    pub fn get(&self, index: usize, output: &mut [f32]) {
        assert!(index < self.count);
        assert_eq!(output.len(), self.dimensions);

        for (dim_idx, out) in output.iter_mut().enumerate().take(self.dimensions) {
            let offset = dim_idx * self.capacity + index;
            *out = unsafe { *self.data.add(offset) };
        }
    }

    /// Get a slice of a specific dimension across all vectors
    /// This allows efficient SIMD operations on a single dimension
    pub fn dimension_slice(&self, dim_idx: usize) -> &[f32] {
        assert!(dim_idx < self.dimensions);
        let offset = dim_idx * self.capacity;
        unsafe { std::slice::from_raw_parts(self.data.add(offset), self.count) }
    }

    /// Get a mutable slice of a specific dimension
    pub fn dimension_slice_mut(&mut self, dim_idx: usize) -> &mut [f32] {
        assert!(dim_idx < self.dimensions);
        let offset = dim_idx * self.capacity;
        unsafe { std::slice::from_raw_parts_mut(self.data.add(offset), self.count) }
    }

    /// Number of vectors stored
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Dimensions per vector
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Grow the storage capacity
    fn grow(&mut self) {
        let new_capacity = self.capacity * 2;

        // Security: Use checked arithmetic to prevent overflow
        let new_total_elements = self
            .dimensions
            .checked_mul(new_capacity)
            .expect("dimensions * new_capacity overflow");
        let new_total_bytes = new_total_elements
            .checked_mul(std::mem::size_of::<f32>())
            .expect("total size overflow in grow");

        let new_layout = Layout::from_size_align(new_total_bytes, CACHE_LINE_SIZE)
            .expect("invalid memory layout in grow");

        let new_data = unsafe { alloc(new_layout) as *mut f32 };

        // Copy old data dimension by dimension
        for dim_idx in 0..self.dimensions {
            let old_offset = dim_idx * self.capacity;
            let new_offset = dim_idx * new_capacity;

            unsafe {
                ptr::copy_nonoverlapping(
                    self.data.add(old_offset),
                    new_data.add(new_offset),
                    self.count,
                );
            }
        }

        // Deallocate old data
        let old_layout = Layout::from_size_align(
            self.dimensions * self.capacity * std::mem::size_of::<f32>(),
            CACHE_LINE_SIZE,
        )
        .unwrap();

        unsafe {
            dealloc(self.data as *mut u8, old_layout);
        }

        self.data = new_data;
        self.capacity = new_capacity;
    }

    /// Compute distance from query to all stored vectors using dimension-wise operations
    /// This takes advantage of the SoA layout for better cache utilization
    #[inline(always)]
    pub fn batch_euclidean_distances(&self, query: &[f32], output: &mut [f32]) {
        assert_eq!(query.len(), self.dimensions);
        assert_eq!(output.len(), self.count);

        // Use SIMD-optimized version for larger batches
        #[cfg(target_arch = "aarch64")]
        {
            if self.count >= 16 {
                unsafe { self.batch_euclidean_distances_neon(query, output) };
                return;
            }
        }

        #[cfg(target_arch = "x86_64")]
        {
            if self.count >= 32 && is_x86_feature_detected!("avx2") {
                unsafe { self.batch_euclidean_distances_avx2(query, output) };
                return;
            }
        }

        // Scalar fallback
        self.batch_euclidean_distances_scalar(query, output);
    }

    /// Scalar implementation of batch euclidean distances
    #[inline(always)]
    fn batch_euclidean_distances_scalar(&self, query: &[f32], output: &mut [f32]) {
        // Initialize output with zeros
        output.fill(0.0);

        // Process dimension by dimension for cache-friendly access
        for dim_idx in 0..self.dimensions {
            let dim_slice = self.dimension_slice(dim_idx);
            // Safety: dim_idx is bounded by self.dimensions which is validated in constructor
            let query_val = unsafe { *query.get_unchecked(dim_idx) };

            // Compute squared differences for this dimension
            // Use unchecked access since vec_idx is bounded by self.count
            for vec_idx in 0..self.count {
                let diff = unsafe { *dim_slice.get_unchecked(vec_idx) } - query_val;
                unsafe { *output.get_unchecked_mut(vec_idx) += diff * diff };
            }
        }

        // Take square root
        for distance in output.iter_mut() {
            *distance = distance.sqrt();
        }
    }

    /// NEON-optimized batch euclidean distances
    ///
    /// # Safety
    /// Caller must ensure query.len() == self.dimensions and output.len() == self.count
    #[cfg(target_arch = "aarch64")]
    #[inline(always)]
    unsafe fn batch_euclidean_distances_neon(&self, query: &[f32], output: &mut [f32]) {
        use std::arch::aarch64::*;

        let out_ptr = output.as_mut_ptr();
        let query_ptr = query.as_ptr();

        // Initialize output with zeros
        let chunks = self.count / 4;

        // Zero initialize using SIMD
        let zero = vdupq_n_f32(0.0);
        for i in 0..chunks {
            let idx = i * 4;
            vst1q_f32(out_ptr.add(idx), zero);
        }
        for i in (chunks * 4)..self.count {
            *output.get_unchecked_mut(i) = 0.0;
        }

        // Process dimension by dimension for cache-friendly access
        for dim_idx in 0..self.dimensions {
            let dim_slice = self.dimension_slice(dim_idx);
            let dim_ptr = dim_slice.as_ptr();
            let query_val = vdupq_n_f32(*query_ptr.add(dim_idx));

            // SIMD processing of 4 vectors at a time
            for i in 0..chunks {
                let idx = i * 4;
                let dim_vals = vld1q_f32(dim_ptr.add(idx));
                let out_vals = vld1q_f32(out_ptr.add(idx));

                let diff = vsubq_f32(dim_vals, query_val);
                let result = vfmaq_f32(out_vals, diff, diff);

                vst1q_f32(out_ptr.add(idx), result);
            }

            // Handle remainder with bounds-check elimination
            let query_val_scalar = *query_ptr.add(dim_idx);
            for i in (chunks * 4)..self.count {
                let diff = *dim_slice.get_unchecked(i) - query_val_scalar;
                *output.get_unchecked_mut(i) += diff * diff;
            }
        }

        // Take square root using SIMD vsqrtq_f32
        for i in 0..chunks {
            let idx = i * 4;
            let vals = vld1q_f32(out_ptr.add(idx));
            let sqrt_vals = vsqrtq_f32(vals);
            vst1q_f32(out_ptr.add(idx), sqrt_vals);
        }
        for i in (chunks * 4)..self.count {
            *output.get_unchecked_mut(i) = output.get_unchecked(i).sqrt();
        }
    }

    /// AVX2-optimized batch euclidean distances
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn batch_euclidean_distances_avx2(&self, query: &[f32], output: &mut [f32]) {
        use std::arch::x86_64::*;

        let chunks = self.count / 8;

        // Zero initialize using SIMD
        let zero = _mm256_setzero_ps();
        for i in 0..chunks {
            let idx = i * 8;
            _mm256_storeu_ps(output.as_mut_ptr().add(idx), zero);
        }
        for out in output.iter_mut().take(self.count).skip(chunks * 8) {
            *out = 0.0;
        }

        // Process dimension by dimension
        for (dim_idx, &q_val) in query.iter().enumerate().take(self.dimensions) {
            let dim_slice = self.dimension_slice(dim_idx);
            let query_val = _mm256_set1_ps(q_val);

            // SIMD processing of 8 vectors at a time
            for i in 0..chunks {
                let idx = i * 8;
                let dim_vals = _mm256_loadu_ps(dim_slice.as_ptr().add(idx));
                let out_vals = _mm256_loadu_ps(output.as_ptr().add(idx));

                let diff = _mm256_sub_ps(dim_vals, query_val);
                let sq = _mm256_mul_ps(diff, diff);
                let result = _mm256_add_ps(out_vals, sq);

                _mm256_storeu_ps(output.as_mut_ptr().add(idx), result);
            }

            // Handle remainder
            for i in (chunks * 8)..self.count {
                let diff = dim_slice[i] - query[dim_idx];
                output[i] += diff * diff;
            }
        }

        // Take square root (no SIMD sqrt in basic AVX2, use scalar)
        for distance in output.iter_mut() {
            *distance = distance.sqrt();
        }
    }
}

// Feature detection helper for x86_64
#[cfg(target_arch = "x86_64")]
#[allow(dead_code)]
fn is_x86_feature_detected_helper(feature: &str) -> bool {
    match feature {
        "avx2" => is_x86_feature_detected!("avx2"),
        _ => false,
    }
}

impl Drop for SoAVectorStorage {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(
            self.dimensions * self.capacity * std::mem::size_of::<f32>(),
            CACHE_LINE_SIZE,
        )
        .unwrap();

        unsafe {
            dealloc(self.data as *mut u8, layout);
        }
    }
}

unsafe impl Send for SoAVectorStorage {}
unsafe impl Sync for SoAVectorStorage {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soa_storage() {
        let mut storage = SoAVectorStorage::new(3, 4);

        storage.push(&[1.0, 2.0, 3.0]);
        storage.push(&[4.0, 5.0, 6.0]);

        assert_eq!(storage.len(), 2);

        let mut output = vec![0.0; 3];
        storage.get(0, &mut output);
        assert_eq!(output, vec![1.0, 2.0, 3.0]);

        storage.get(1, &mut output);
        assert_eq!(output, vec![4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_dimension_slice() {
        let mut storage = SoAVectorStorage::new(3, 4);

        storage.push(&[1.0, 2.0, 3.0]);
        storage.push(&[4.0, 5.0, 6.0]);
        storage.push(&[7.0, 8.0, 9.0]);

        // Get all values for dimension 0
        let dim0 = storage.dimension_slice(0);
        assert_eq!(dim0, &[1.0, 4.0, 7.0]);

        // Get all values for dimension 1
        let dim1 = storage.dimension_slice(1);
        assert_eq!(dim1, &[2.0, 5.0, 8.0]);
    }

    #[test]
    fn test_batch_distances() {
        let mut storage = SoAVectorStorage::new(3, 4);

        storage.push(&[1.0, 0.0, 0.0]);
        storage.push(&[0.0, 1.0, 0.0]);
        storage.push(&[0.0, 0.0, 1.0]);

        let query = vec![1.0, 0.0, 0.0];
        let mut distances = vec![0.0; 3];

        storage.batch_euclidean_distances(&query, &mut distances);

        assert!((distances[0] - 0.0).abs() < 0.001);
        assert!((distances[1] - 1.414).abs() < 0.01);
        assert!((distances[2] - 1.414).abs() < 0.01);
    }
}
