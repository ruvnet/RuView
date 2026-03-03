//! Optimized sparse matrix implementations with SIMD acceleration and buffer pooling.
//!
//! This module provides high-performance matrix storage formats optimized for
//! sublinear-time algorithms with focus on minimizing memory allocation overhead
//! and maximizing cache efficiency.

use crate::types::{Precision, DimensionType, IndexType};
use crate::error::{SolverError, Result};
use crate::matrix::sparse::{CSRStorage, CSCStorage, COOStorage};
use alloc::{vec::Vec, collections::VecDeque, boxed::Box};
use core::sync::atomic::{AtomicUsize, Ordering};

#[cfg(feature = "std")]
use std::sync::Mutex;

#[cfg(feature = "simd")]
use wide::f64x4;

/// High-performance buffer pool for reducing allocation overhead.
///
/// This pool maintains pre-allocated buffers of various sizes to minimize
/// runtime allocations during matrix operations.
pub struct BufferPool {
    /// Small buffers (< 1KB)
    small_buffers: VecDeque<Vec<Precision>>,
    /// Medium buffers (1KB - 64KB)
    medium_buffers: VecDeque<Vec<Precision>>,
    /// Large buffers (> 64KB)
    large_buffers: VecDeque<Vec<Precision>>,
    /// Statistics
    allocations: AtomicUsize,
    deallocations: AtomicUsize,
    cache_hits: AtomicUsize,
    cache_misses: AtomicUsize,
}

/// Buffer size categories
const SMALL_BUFFER_THRESHOLD: usize = 128;  // 1KB for f64
const MEDIUM_BUFFER_THRESHOLD: usize = 8192; // 64KB for f64

impl BufferPool {
    /// Create a new buffer pool with initial capacity.
    pub fn new() -> Self {
        Self {
            small_buffers: VecDeque::with_capacity(16),
            medium_buffers: VecDeque::with_capacity(8),
            large_buffers: VecDeque::with_capacity(4),
            allocations: AtomicUsize::new(0),
            deallocations: AtomicUsize::new(0),
            cache_hits: AtomicUsize::new(0),
            cache_misses: AtomicUsize::new(0),
        }
    }

    /// Get a buffer of at least the requested size.
    pub fn get_buffer(&mut self, min_size: usize) -> Vec<Precision> {
        self.allocations.fetch_add(1, Ordering::Relaxed);

        let buffer_queue = if min_size <= SMALL_BUFFER_THRESHOLD {
            &mut self.small_buffers
        } else if min_size <= MEDIUM_BUFFER_THRESHOLD {
            &mut self.medium_buffers
        } else {
            &mut self.large_buffers
        };

        // Try to find a suitable buffer
        for _ in 0..buffer_queue.len() {
            if let Some(mut buffer) = buffer_queue.pop_front() {
                if buffer.capacity() >= min_size {
                    buffer.clear();
                    buffer.resize(min_size, 0.0);
                    self.cache_hits.fetch_add(1, Ordering::Relaxed);
                    return buffer;
                } else {
                    buffer_queue.push_back(buffer);
                }
            }
        }

        // No suitable buffer found, allocate new
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
        vec![0.0; min_size]
    }

    /// Return a buffer to the pool.
    pub fn return_buffer(&mut self, buffer: Vec<Precision>) {
        self.deallocations.fetch_add(1, Ordering::Relaxed);

        let capacity = buffer.capacity();
        let buffer_queue = if capacity <= SMALL_BUFFER_THRESHOLD {
            &mut self.small_buffers
        } else if capacity <= MEDIUM_BUFFER_THRESHOLD {
            &mut self.medium_buffers
        } else {
            &mut self.large_buffers
        };

        // Only store if we have room and the buffer is reasonable size
        if buffer_queue.len() < 32 && capacity < 1_000_000 {
            buffer_queue.push_back(buffer);
        }
        // Otherwise let it drop
    }

    /// Get buffer pool statistics.
    pub fn stats(&self) -> BufferPoolStats {
        BufferPoolStats {
            allocations: self.allocations.load(Ordering::Relaxed),
            deallocations: self.deallocations.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            small_buffers_pooled: self.small_buffers.len(),
            medium_buffers_pooled: self.medium_buffers.len(),
            large_buffers_pooled: self.large_buffers.len(),
        }
    }

    /// Clear all pooled buffers to free memory.
    pub fn clear(&mut self) {
        self.small_buffers.clear();
        self.medium_buffers.clear();
        self.large_buffers.clear();
    }
}

impl Default for BufferPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Buffer pool statistics.
#[derive(Debug, Clone, Copy)]
pub struct BufferPoolStats {
    pub allocations: usize,
    pub deallocations: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub small_buffers_pooled: usize,
    pub medium_buffers_pooled: usize,
    pub large_buffers_pooled: usize,
}

impl BufferPoolStats {
    /// Calculate cache hit rate as a percentage.
    pub fn hit_rate(&self) -> f64 {
        if self.allocations == 0 {
            0.0
        } else {
            (self.cache_hits as f64 / self.allocations as f64) * 100.0
        }
    }
}

/// Thread-safe global buffer pool.
#[cfg(all(feature = "std", feature = "lazy_static"))]
lazy_static::lazy_static! {
    static ref GLOBAL_BUFFER_POOL: Mutex<BufferPool> = Mutex::new(BufferPool::new());
}

/// Get a buffer from the global pool.
#[cfg(all(feature = "std", feature = "lazy_static"))]
pub fn get_global_buffer(min_size: usize) -> Vec<Precision> {
    GLOBAL_BUFFER_POOL.lock().unwrap().get_buffer(min_size)
}

/// Return a buffer to the global pool.
#[cfg(all(feature = "std", feature = "lazy_static"))]
pub fn return_global_buffer(buffer: Vec<Precision>) {
    GLOBAL_BUFFER_POOL.lock().unwrap().return_buffer(buffer);
}

/// Optimized CSR storage with SIMD acceleration and buffer pooling.
pub struct OptimizedCSRStorage {
    /// Base CSR storage
    storage: CSRStorage,
    /// Buffer pool for temporary vectors
    buffer_pool: BufferPool,
    /// Pre-allocated workspace
    workspace: Vec<Precision>,
    /// Performance counters
    matvec_count: AtomicUsize,
    bytes_processed: AtomicUsize,
}

impl OptimizedCSRStorage {
    /// Create optimized CSR storage from COO format.
    pub fn from_coo(coo: &COOStorage, rows: DimensionType, cols: DimensionType) -> Result<Self> {
        let storage = CSRStorage::from_coo(coo, rows, cols)?;
        let workspace_size = rows.max(cols);

        Ok(Self {
            storage,
            buffer_pool: BufferPool::new(),
            workspace: vec![0.0; workspace_size],
            matvec_count: AtomicUsize::new(0),
            bytes_processed: AtomicUsize::new(0),
        })
    }

    /// SIMD-accelerated matrix-vector multiplication.
    #[cfg(feature = "simd")]
    pub fn multiply_vector_simd(&self, x: &[Precision], result: &mut [Precision]) {
        result.fill(0.0);
        self.matvec_count.fetch_add(1, Ordering::Relaxed);

        let bytes = (self.storage.values.len() * 8) + (x.len() * 8) + (result.len() * 8);
        self.bytes_processed.fetch_add(bytes, Ordering::Relaxed);

        for (row, row_result) in result.iter_mut().enumerate() {
            let start = self.storage.row_ptr[row] as usize;
            let end = self.storage.row_ptr[row + 1] as usize;

            if end <= start {
                continue;
            }

            let row_values = &self.storage.values[start..end];
            let row_indices = &self.storage.col_indices[start..end];

            // Process in chunks of 4 for SIMD
            let simd_chunks = row_values.len() / 4;
            let mut sum = f64x4::splat(0.0);

            for chunk in 0..simd_chunks {
                let val_idx = chunk * 4;
                let values = f64x4::new([
                    row_values[val_idx],
                    row_values[val_idx + 1],
                    row_values[val_idx + 2],
                    row_values[val_idx + 3],
                ]);

                let x_vals = f64x4::new([
                    x[row_indices[val_idx] as usize],
                    x[row_indices[val_idx + 1] as usize],
                    x[row_indices[val_idx + 2] as usize],
                    x[row_indices[val_idx + 3] as usize],
                ]);

                sum = sum + (values * x_vals);
            }

            // Sum the SIMD register
            let sum_array = sum.to_array();
            *row_result = sum_array[0] + sum_array[1] + sum_array[2] + sum_array[3];

            // Handle remaining elements
            for i in (simd_chunks * 4)..row_values.len() {
                let col = row_indices[i] as usize;
                *row_result += row_values[i] * x[col];
            }
        }
    }

    /// Fallback non-SIMD matrix-vector multiplication.
    #[cfg(not(feature = "simd"))]
    pub fn multiply_vector_simd(&self, x: &[Precision], result: &mut [Precision]) {
        self.multiply_vector_optimized(x, result);
    }

    /// Cache-optimized matrix-vector multiplication.
    pub fn multiply_vector_optimized(&self, x: &[Precision], result: &mut [Precision]) {
        result.fill(0.0);
        self.matvec_count.fetch_add(1, Ordering::Relaxed);

        // Use blocked computation for better cache behavior
        const BLOCK_SIZE: usize = 64; // Chosen for L1 cache efficiency

        for row_block in (0..result.len()).step_by(BLOCK_SIZE) {
            let row_end = (row_block + BLOCK_SIZE).min(result.len());

            for row in row_block..row_end {
                let start = self.storage.row_ptr[row] as usize;
                let end = self.storage.row_ptr[row + 1] as usize;

                let mut sum = 0.0;
                for i in start..end {
                    let col = self.storage.col_indices[i] as usize;
                    sum += self.storage.values[i] * x[col];
                }
                result[row] = sum;
            }
        }
    }

    /// Streaming matrix-vector multiplication for large matrices.
    pub fn multiply_vector_streaming<F>(
        &self,
        x: &[Precision],
        mut callback: F,
        chunk_size: usize
    ) -> Result<()>
    where
        F: FnMut(usize, &[Precision]),
    {
        let mut result_chunk = vec![0.0; chunk_size];

        for chunk_start in (0..self.storage.row_ptr.len() - 1).step_by(chunk_size) {
            let chunk_end = (chunk_start + chunk_size).min(self.storage.row_ptr.len() - 1);
            let actual_chunk_size = chunk_end - chunk_start;

            result_chunk.resize(actual_chunk_size, 0.0);
            result_chunk.fill(0.0);

            // Compute this chunk
            for (local_row, global_row) in (chunk_start..chunk_end).enumerate() {
                let start = self.storage.row_ptr[global_row] as usize;
                let end = self.storage.row_ptr[global_row + 1] as usize;

                let mut sum = 0.0;
                for i in start..end {
                    let col = self.storage.col_indices[i] as usize;
                    sum += self.storage.values[i] * x[col];
                }
                result_chunk[local_row] = sum;
            }

            callback(chunk_start, &result_chunk[..actual_chunk_size]);
        }

        Ok(())
    }

    /// Get performance statistics.
    pub fn performance_stats(&self) -> OptimizedMatrixStats {
        OptimizedMatrixStats {
            matvec_count: self.matvec_count.load(Ordering::Relaxed),
            bytes_processed: self.bytes_processed.load(Ordering::Relaxed),
            buffer_pool_stats: self.buffer_pool.stats(),
            matrix_nnz: self.storage.nnz(),
            matrix_rows: self.storage.row_ptr.len() - 1,
            workspace_size: self.workspace.len(),
        }
    }

    /// Reset performance counters.
    pub fn reset_stats(&self) {
        self.matvec_count.store(0, Ordering::Relaxed);
        self.bytes_processed.store(0, Ordering::Relaxed);
    }

    /// Get a temporary buffer from the pool.
    pub fn get_temp_buffer(&mut self, size: usize) -> Vec<Precision> {
        self.buffer_pool.get_buffer(size)
    }

    /// Return a temporary buffer to the pool.
    pub fn return_temp_buffer(&mut self, buffer: Vec<Precision>) {
        self.buffer_pool.return_buffer(buffer);
    }

    /// Access the underlying CSR storage.
    pub fn storage(&self) -> &CSRStorage {
        &self.storage
    }
}

/// Performance statistics for optimized matrix operations.
#[derive(Debug, Clone)]
pub struct OptimizedMatrixStats {
    pub matvec_count: usize,
    pub bytes_processed: usize,
    pub buffer_pool_stats: BufferPoolStats,
    pub matrix_nnz: usize,
    pub matrix_rows: usize,
    pub workspace_size: usize,
}

impl OptimizedMatrixStats {
    /// Calculate effective bandwidth in GB/s.
    pub fn bandwidth_gbs(&self, total_time_ms: f64) -> f64 {
        if total_time_ms <= 0.0 {
            0.0
        } else {
            let total_gb = self.bytes_processed as f64 / 1_073_741_824.0; // Convert to GB
            let total_seconds = total_time_ms / 1000.0;
            total_gb / total_seconds
        }
    }

    /// Calculate operations per second.
    pub fn ops_per_second(&self, total_time_ms: f64) -> f64 {
        if total_time_ms <= 0.0 {
            0.0
        } else {
            let total_ops = self.matvec_count as f64;
            let total_seconds = total_time_ms / 1000.0;
            total_ops / total_seconds
        }
    }
}

/// Parallel CSR storage for multi-threaded operations.
#[cfg(feature = "std")]
pub struct ParallelCSRStorage {
    storage: OptimizedCSRStorage,
    num_threads: usize,
}

#[cfg(feature = "std")]
impl ParallelCSRStorage {
    /// Create parallel CSR storage.
    pub fn new(storage: OptimizedCSRStorage, num_threads: Option<usize>) -> Self {
        let num_threads = num_threads.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(1)
        });

        Self {
            storage,
            num_threads,
        }
    }

    /// Parallel matrix-vector multiplication using Rayon.
    #[cfg(feature = "rayon")]
    pub fn multiply_vector_parallel(&self, x: &[Precision], result: &mut [Precision]) {
        use rayon::prelude::*;

        result.fill(0.0);

        // Determine chunk size for good load balancing
        let rows = result.len();
        let chunk_size = (rows + self.num_threads - 1) / self.num_threads;

        result.par_chunks_mut(chunk_size)
            .enumerate()
            .for_each(|(chunk_idx, result_chunk)| {
                let start_row = chunk_idx * chunk_size;
                let end_row = (start_row + result_chunk.len()).min(rows);

                for (local_idx, global_row) in (start_row..end_row).enumerate() {
                    let start = self.storage.storage.row_ptr[global_row] as usize;
                    let end = self.storage.storage.row_ptr[global_row + 1] as usize;

                    let mut sum = 0.0;
                    for i in start..end {
                        let col = self.storage.storage.col_indices[i] as usize;
                        sum += self.storage.storage.values[i] * x[col];
                    }
                    result_chunk[local_idx] = sum;
                }
            });
    }
}

/// Memory-efficient matrix representation for extremely large problems.
pub struct StreamingMatrix {
    /// Matrix stored in chunks
    chunks: Vec<OptimizedCSRStorage>,
    /// Chunk size (number of rows per chunk)
    chunk_size: usize,
    /// Total dimensions
    total_rows: usize,
    total_cols: usize,
    /// Memory limit in bytes
    memory_limit: usize,
}

impl StreamingMatrix {
    /// Create a streaming matrix from triplets with memory constraints.
    pub fn from_triplets(
        triplets: Vec<(usize, usize, Precision)>,
        rows: usize,
        cols: usize,
        memory_limit_mb: usize,
    ) -> Result<Self> {
        let memory_limit = memory_limit_mb * 1_048_576; // Convert to bytes

        // Estimate memory per row
        let nnz = triplets.len();
        let avg_nnz_per_row = if rows > 0 { nnz / rows } else { 0 };
        let bytes_per_row = avg_nnz_per_row * (8 + 4) + 4; // value + col_index + row_ptr

        // Calculate chunk size to stay within memory limit
        let target_chunk_size = if bytes_per_row > 0 {
            (memory_limit / (bytes_per_row * 2)).max(1) // Factor of 2 for safety
        } else {
            1000
        };

        let chunk_size = target_chunk_size.min(rows);

        // Sort triplets by row
        let mut sorted_triplets = triplets;
        sorted_triplets.sort_by_key(|(row, _, _)| *row);

        // Split into chunks
        let mut chunks = Vec::new();
        let num_chunks = (rows + chunk_size - 1) / chunk_size;

        for chunk_idx in 0..num_chunks {
            let chunk_start_row = chunk_idx * chunk_size;
            let chunk_end_row = ((chunk_idx + 1) * chunk_size).min(rows);
            let chunk_rows = chunk_end_row - chunk_start_row;

            // Extract triplets for this chunk
            let chunk_triplets: Vec<(usize, usize, Precision)> = sorted_triplets
                .iter()
                .filter(|(row, _, _)| *row >= chunk_start_row && *row < chunk_end_row)
                .map(|(row, col, val)| (row - chunk_start_row, *col, *val))
                .collect();

            // Create chunk storage
            if !chunk_triplets.is_empty() {
                let coo = COOStorage::from_triplets(chunk_triplets)?;
                let chunk_storage = OptimizedCSRStorage::from_coo(&coo, chunk_rows, cols)?;
                chunks.push(chunk_storage);
            } else {
                // Empty chunk
                let empty_coo = COOStorage::from_triplets(vec![])?;
                let empty_storage = OptimizedCSRStorage::from_coo(&empty_coo, chunk_rows, cols)?;
                chunks.push(empty_storage);
            }
        }

        Ok(Self {
            chunks,
            chunk_size,
            total_rows: rows,
            total_cols: cols,
            memory_limit,
        })
    }

    /// Streaming matrix-vector multiplication.
    pub fn multiply_vector_streaming<F>(
        &self,
        x: &[Precision],
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(usize, &[Precision]),
    {
        for (chunk_idx, chunk) in self.chunks.iter().enumerate() {
            let start_row = chunk_idx * self.chunk_size;
            let end_row = (start_row + self.chunk_size).min(self.total_rows);
            let chunk_rows = end_row - start_row;

            let mut result = vec![0.0; chunk_rows];
            chunk.multiply_vector_optimized(x, &mut result);

            callback(start_row, &result);
        }

        Ok(())
    }

    /// Get memory usage statistics.
    pub fn memory_usage(&self) -> usize {
        self.chunks.iter()
            .map(|chunk| {
                let stats = chunk.performance_stats();
                stats.matrix_nnz * 12 + stats.matrix_rows * 4 // Rough estimate
            })
            .sum()
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool() {
        let mut pool = BufferPool::new();

        // Test buffer allocation and return
        let buffer1 = pool.get_buffer(100);
        assert_eq!(buffer1.len(), 100);

        pool.return_buffer(buffer1);

        let buffer2 = pool.get_buffer(50);
        assert_eq!(buffer2.len(), 50);

        let stats = pool.stats();
        assert_eq!(stats.allocations, 2);
        assert_eq!(stats.deallocations, 1);
    }

    #[test]
    fn test_optimized_csr_performance() {
        // Create a simple test matrix
        let triplets = vec![
            (0, 0, 2.0), (0, 1, 1.0),
            (1, 0, 1.0), (1, 1, 3.0),
        ];
        let coo = COOStorage::from_triplets(triplets).unwrap();
        let optimized = OptimizedCSRStorage::from_coo(&coo, 2, 2).unwrap();

        let x = vec![1.0, 2.0];
        let mut result = vec![0.0; 2];

        optimized.multiply_vector_optimized(&x, &mut result);
        assert_eq!(result, vec![4.0, 7.0]);

        let stats = optimized.performance_stats();
        assert_eq!(stats.matvec_count, 1);
    }

    #[test]
    fn test_streaming_matrix() {
        let triplets = vec![
            (0, 0, 1.0), (0, 1, 2.0),
            (1, 0, 3.0), (1, 1, 4.0),
            (2, 0, 5.0), (2, 1, 6.0),
        ];

        let streaming = StreamingMatrix::from_triplets(triplets, 3, 2, 1).unwrap();
        let x = vec![1.0, 1.0];

        let mut results = Vec::new();
        streaming.multiply_vector_streaming(&x, |start_row, chunk_result| {
            results.extend_from_slice(chunk_result);
        }).unwrap();

        // Each chunk should produce correct results
        assert!(results.len() >= 3);
    }
}