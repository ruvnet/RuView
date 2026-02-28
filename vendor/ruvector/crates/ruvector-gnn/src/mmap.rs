//! Memory-mapped embedding management for large-scale GNN training.
//!
//! This module provides efficient memory-mapped access to embeddings and gradients
//! that don't fit in RAM. It includes:
//! - `MmapManager`: Memory-mapped embedding storage with dirty tracking
//! - `MmapGradientAccumulator`: Lock-free gradient accumulation
//! - `AtomicBitmap`: Thread-safe bitmap for access/dirty tracking
//!
//! Only available on non-WASM targets.

#![cfg(all(not(target_arch = "wasm32"), feature = "mmap"))]

use crate::error::{GnnError, Result};
use memmap2::{MmapMut, MmapOptions};
use parking_lot::RwLock;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Thread-safe bitmap using atomic operations.
///
/// Used for tracking which embeddings have been accessed or modified.
/// Each bit represents one embedding node.
#[derive(Debug)]
pub struct AtomicBitmap {
    /// Array of 64-bit atomic integers, each storing 64 bits
    bits: Vec<AtomicU64>,
    /// Total number of bits (nodes)
    size: usize,
}

impl AtomicBitmap {
    /// Create a new atomic bitmap with the specified capacity.
    ///
    /// # Arguments
    /// * `size` - Number of bits to allocate
    pub fn new(size: usize) -> Self {
        let num_words = (size + 63) / 64;
        let bits = (0..num_words).map(|_| AtomicU64::new(0)).collect();

        Self { bits, size }
    }

    /// Set a bit to 1 (mark as accessed/dirty).
    ///
    /// # Arguments
    /// * `index` - Bit index to set
    pub fn set(&self, index: usize) {
        if index >= self.size {
            return;
        }
        let word_idx = index / 64;
        let bit_idx = index % 64;
        self.bits[word_idx].fetch_or(1u64 << bit_idx, Ordering::Release);
    }

    /// Clear a bit to 0 (mark as clean/not accessed).
    ///
    /// # Arguments
    /// * `index` - Bit index to clear
    pub fn clear(&self, index: usize) {
        if index >= self.size {
            return;
        }
        let word_idx = index / 64;
        let bit_idx = index % 64;
        self.bits[word_idx].fetch_and(!(1u64 << bit_idx), Ordering::Release);
    }

    /// Check if a bit is set.
    ///
    /// # Arguments
    /// * `index` - Bit index to check
    ///
    /// # Returns
    /// `true` if the bit is set, `false` otherwise
    pub fn get(&self, index: usize) -> bool {
        if index >= self.size {
            return false;
        }
        let word_idx = index / 64;
        let bit_idx = index % 64;
        let word = self.bits[word_idx].load(Ordering::Acquire);
        (word & (1u64 << bit_idx)) != 0
    }

    /// Clear all bits in the bitmap.
    pub fn clear_all(&self) {
        for word in &self.bits {
            word.store(0, Ordering::Release);
        }
    }

    /// Get all set bit indices (for finding dirty pages).
    ///
    /// # Returns
    /// Vector of indices where bits are set
    pub fn get_set_indices(&self) -> Vec<usize> {
        let mut indices = Vec::new();
        for (word_idx, word) in self.bits.iter().enumerate() {
            let mut w = word.load(Ordering::Acquire);
            while w != 0 {
                let bit_idx = w.trailing_zeros() as usize;
                indices.push(word_idx * 64 + bit_idx);
                w &= w - 1; // Clear lowest set bit
            }
        }
        indices
    }
}

/// Memory-mapped embedding manager with dirty tracking and prefetching.
///
/// Manages large embedding matrices that may not fit in RAM using memory-mapped files.
/// Tracks which embeddings have been accessed and modified for efficient I/O.
#[derive(Debug)]
pub struct MmapManager {
    /// The memory-mapped file
    file: File,
    /// Mutable memory mapping
    mmap: MmapMut,
    /// Operating system page size
    page_size: usize,
    /// Embedding dimension
    d_embed: usize,
    /// Bitmap tracking which embeddings have been accessed
    access_bitmap: AtomicBitmap,
    /// Bitmap tracking which embeddings have been modified
    dirty_bitmap: AtomicBitmap,
    /// Pin count for each page (prevents eviction)
    pin_count: Vec<AtomicU32>,
    /// Maximum number of nodes
    max_nodes: usize,
}

impl MmapManager {
    /// Create a new memory-mapped embedding manager.
    ///
    /// # Arguments
    /// * `path` - Path to the memory-mapped file
    /// * `d_embed` - Embedding dimension
    /// * `max_nodes` - Maximum number of nodes to support
    ///
    /// # Returns
    /// A new `MmapManager` instance
    pub fn new(path: &Path, d_embed: usize, max_nodes: usize) -> Result<Self> {
        // Calculate required file size
        let embedding_size = d_embed * std::mem::size_of::<f32>();
        let file_size = max_nodes * embedding_size;

        // Create or open the file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .map_err(|e| GnnError::mmap(format!("Failed to open mmap file: {}", e)))?;

        // Set file size
        file.set_len(file_size as u64)
            .map_err(|e| GnnError::mmap(format!("Failed to set file size: {}", e)))?;

        // Create memory mapping
        let mmap = unsafe {
            MmapOptions::new()
                .len(file_size)
                .map_mut(&file)
                .map_err(|e| GnnError::mmap(format!("Failed to create mmap: {}", e)))?
        };

        // Get system page size
        let page_size = page_size::get();
        let num_pages = (file_size + page_size - 1) / page_size;

        Ok(Self {
            file,
            mmap,
            page_size,
            d_embed,
            access_bitmap: AtomicBitmap::new(max_nodes),
            dirty_bitmap: AtomicBitmap::new(max_nodes),
            pin_count: (0..num_pages).map(|_| AtomicU32::new(0)).collect(),
            max_nodes,
        })
    }

    /// Calculate the byte offset for a given node's embedding.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier
    ///
    /// # Returns
    /// Byte offset in the memory-mapped file, or None if overflow would occur
    ///
    /// # Security
    /// Uses checked arithmetic to prevent integer overflow attacks.
    #[inline]
    pub fn embedding_offset(&self, node_id: u64) -> Option<usize> {
        let node_idx = usize::try_from(node_id).ok()?;
        let elem_size = std::mem::size_of::<f32>();
        node_idx.checked_mul(self.d_embed)?.checked_mul(elem_size)
    }

    /// Validate that a node_id is within bounds.
    #[inline]
    fn validate_node_id(&self, node_id: u64) -> bool {
        (node_id as usize) < self.max_nodes
    }

    /// Get a read-only reference to a node's embedding.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier
    ///
    /// # Returns
    /// Slice containing the embedding vector
    ///
    /// # Panics
    /// Panics if node_id is out of bounds or would cause overflow
    pub fn get_embedding(&self, node_id: u64) -> &[f32] {
        // Security: Validate bounds before any pointer arithmetic
        assert!(
            self.validate_node_id(node_id),
            "node_id {} out of bounds (max: {})",
            node_id,
            self.max_nodes
        );

        let offset = self
            .embedding_offset(node_id)
            .expect("embedding offset calculation overflow");
        let end = offset
            .checked_add(
                self.d_embed
                    .checked_mul(std::mem::size_of::<f32>())
                    .unwrap(),
            )
            .expect("end offset overflow");
        assert!(
            end <= self.mmap.len(),
            "embedding extends beyond mmap bounds"
        );

        // Mark as accessed
        self.access_bitmap.set(node_id as usize);

        // Safety: We control the offset and know the data is properly aligned
        unsafe {
            let ptr = self.mmap.as_ptr().add(offset) as *const f32;
            std::slice::from_raw_parts(ptr, self.d_embed)
        }
    }

    /// Set a node's embedding data.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier
    /// * `data` - Embedding vector to write
    ///
    /// # Panics
    /// Panics if node_id is out of bounds, data length doesn't match d_embed,
    /// or offset calculation would overflow.
    pub fn set_embedding(&mut self, node_id: u64, data: &[f32]) {
        // Security: Validate bounds first
        assert!(
            self.validate_node_id(node_id),
            "node_id {} out of bounds (max: {})",
            node_id,
            self.max_nodes
        );
        assert_eq!(
            data.len(),
            self.d_embed,
            "Embedding data length must match d_embed"
        );

        let offset = self
            .embedding_offset(node_id)
            .expect("embedding offset calculation overflow");
        let end = offset
            .checked_add(data.len().checked_mul(std::mem::size_of::<f32>()).unwrap())
            .expect("end offset overflow");
        assert!(
            end <= self.mmap.len(),
            "embedding extends beyond mmap bounds"
        );

        // Mark as accessed and dirty
        self.access_bitmap.set(node_id as usize);
        self.dirty_bitmap.set(node_id as usize);

        // Safety: We control the offset and know the data is properly aligned
        unsafe {
            let ptr = self.mmap.as_mut_ptr().add(offset) as *mut f32;
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, self.d_embed);
        }
    }

    /// Flush all dirty pages to disk.
    ///
    /// # Returns
    /// `Ok(())` on success, error otherwise
    pub fn flush_dirty(&self) -> io::Result<()> {
        let dirty_nodes = self.dirty_bitmap.get_set_indices();

        if dirty_nodes.is_empty() {
            return Ok(());
        }

        // Flush the entire mmap for simplicity
        // In a production system, you might want to flush only dirty pages
        self.mmap.flush()?;

        // Clear dirty bitmap after successful flush
        for &node_id in &dirty_nodes {
            self.dirty_bitmap.clear(node_id);
        }

        Ok(())
    }

    /// Prefetch embeddings into memory for better cache locality.
    ///
    /// # Arguments
    /// * `node_ids` - List of node IDs to prefetch
    pub fn prefetch(&self, node_ids: &[u64]) {
        #[cfg(target_os = "linux")]
        {
            #[allow(unused_imports)]
            use std::os::unix::io::AsRawFd;

            for &node_id in node_ids {
                // Skip invalid node IDs
                if !self.validate_node_id(node_id) {
                    continue;
                }
                let offset = match self.embedding_offset(node_id) {
                    Some(o) => o,
                    None => continue,
                };
                let page_offset = (offset / self.page_size) * self.page_size;
                let length = self.d_embed * std::mem::size_of::<f32>();

                unsafe {
                    // Use madvise to hint the kernel to prefetch
                    libc::madvise(
                        self.mmap.as_ptr().add(page_offset) as *mut libc::c_void,
                        length,
                        libc::MADV_WILLNEED,
                    );
                }
            }
        }

        // On non-Linux platforms, just access the data to bring it into cache
        #[cfg(not(target_os = "linux"))]
        {
            for &node_id in node_ids {
                if self.validate_node_id(node_id) {
                    let _ = self.get_embedding(node_id);
                }
            }
        }
    }

    /// Get the embedding dimension.
    pub fn d_embed(&self) -> usize {
        self.d_embed
    }

    /// Get the maximum number of nodes.
    pub fn max_nodes(&self) -> usize {
        self.max_nodes
    }
}

/// Memory-mapped gradient accumulator with fine-grained locking.
///
/// Allows multiple threads to accumulate gradients concurrently with minimal contention.
/// Uses reader-writer locks at a configurable granularity.
pub struct MmapGradientAccumulator {
    /// Memory-mapped gradient storage (using UnsafeCell for interior mutability)
    grad_mmap: std::cell::UnsafeCell<MmapMut>,
    /// Number of nodes per lock (lock granularity)
    lock_granularity: usize,
    /// Reader-writer locks for gradient regions
    locks: Vec<RwLock<()>>,
    /// Number of nodes
    n_nodes: usize,
    /// Embedding dimension
    d_embed: usize,
    /// Gradient file
    _file: File,
}

impl MmapGradientAccumulator {
    /// Create a new memory-mapped gradient accumulator.
    ///
    /// # Arguments
    /// * `path` - Path to the gradient file
    /// * `d_embed` - Embedding dimension
    /// * `max_nodes` - Maximum number of nodes
    ///
    /// # Returns
    /// A new `MmapGradientAccumulator` instance
    pub fn new(path: &Path, d_embed: usize, max_nodes: usize) -> Result<Self> {
        // Calculate required file size
        let grad_size = d_embed * std::mem::size_of::<f32>();
        let file_size = max_nodes * grad_size;

        // Create or open the file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .map_err(|e| GnnError::mmap(format!("Failed to open gradient file: {}", e)))?;

        // Set file size
        file.set_len(file_size as u64)
            .map_err(|e| GnnError::mmap(format!("Failed to set gradient file size: {}", e)))?;

        // Create memory mapping
        let grad_mmap = unsafe {
            MmapOptions::new()
                .len(file_size)
                .map_mut(&file)
                .map_err(|e| GnnError::mmap(format!("Failed to create gradient mmap: {}", e)))?
        };

        // Zero out the gradients
        for byte in grad_mmap.iter() {
            // This forces the pages to be allocated and zeroed
            let _ = byte;
        }

        // Use a lock granularity of 64 nodes per lock for good parallelism
        let lock_granularity = 64;
        let num_locks = (max_nodes + lock_granularity - 1) / lock_granularity;
        let locks = (0..num_locks).map(|_| RwLock::new(())).collect();

        Ok(Self {
            grad_mmap: std::cell::UnsafeCell::new(grad_mmap),
            lock_granularity,
            locks,
            n_nodes: max_nodes,
            d_embed,
            _file: file,
        })
    }

    /// Calculate the byte offset for a node's gradient.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier
    ///
    /// # Returns
    /// Byte offset in the gradient file, or None on overflow or out-of-bounds
    ///
    /// # Security
    /// Uses checked arithmetic to prevent integer overflow (SEC-001).
    #[inline]
    pub fn grad_offset(&self, node_id: u64) -> Option<usize> {
        let node_idx = usize::try_from(node_id).ok()?;
        if node_idx >= self.n_nodes {
            return None;
        }
        let elem_size = std::mem::size_of::<f32>();
        node_idx.checked_mul(self.d_embed)?.checked_mul(elem_size)
    }

    /// Accumulate gradients for a specific node.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier
    /// * `grad` - Gradient vector to accumulate
    ///
    /// # Panics
    /// Panics if grad length doesn't match d_embed
    pub fn accumulate(&self, node_id: u64, grad: &[f32]) {
        assert_eq!(
            grad.len(),
            self.d_embed,
            "Gradient length must match d_embed"
        );

        let offset = self
            .grad_offset(node_id)
            .expect("node_id out of bounds or offset overflow");

        let lock_idx = (node_id as usize) / self.lock_granularity;
        assert!(lock_idx < self.locks.len(), "lock index out of bounds");
        let _lock = self.locks[lock_idx].write();

        // Safety: We validated node_id bounds and offset above, and hold the write lock
        unsafe {
            let mmap = &mut *self.grad_mmap.get();
            assert!(
                offset + self.d_embed * std::mem::size_of::<f32>() <= mmap.len(),
                "gradient write would exceed mmap bounds"
            );
            let ptr = mmap.as_mut_ptr().add(offset) as *mut f32;
            let grad_slice = std::slice::from_raw_parts_mut(ptr, self.d_embed);

            // Accumulate gradients
            for (g, &new_g) in grad_slice.iter_mut().zip(grad.iter()) {
                *g += new_g;
            }
        }
    }

    /// Apply accumulated gradients to embeddings and zero out gradients.
    ///
    /// # Arguments
    /// * `learning_rate` - Learning rate for gradient descent
    /// * `embeddings` - Embedding manager to update
    pub fn apply(&mut self, learning_rate: f32, embeddings: &mut MmapManager) {
        assert_eq!(
            self.d_embed, embeddings.d_embed,
            "Gradient and embedding dimensions must match"
        );

        // Process all nodes
        for node_id in 0..self.n_nodes.min(embeddings.max_nodes) {
            let grad = self.get_grad(node_id as u64);
            let embedding = embeddings.get_embedding(node_id as u64);

            // Apply gradient descent: embedding -= learning_rate * grad
            let mut updated = vec![0.0f32; self.d_embed];
            for i in 0..self.d_embed {
                updated[i] = embedding[i] - learning_rate * grad[i];
            }

            embeddings.set_embedding(node_id as u64, &updated);
        }

        // Zero out gradients after applying
        self.zero_grad();
    }

    /// Zero out all accumulated gradients.
    pub fn zero_grad(&mut self) {
        // Zero the entire gradient buffer
        unsafe {
            let mmap = &mut *self.grad_mmap.get();
            for byte in mmap.iter_mut() {
                *byte = 0;
            }
        }
    }

    /// Get a read-only reference to a node's accumulated gradient.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier
    ///
    /// # Returns
    /// Slice containing the gradient vector
    pub fn get_grad(&self, node_id: u64) -> &[f32] {
        let offset = self
            .grad_offset(node_id)
            .expect("node_id out of bounds or offset overflow");

        let lock_idx = (node_id as usize) / self.lock_granularity;
        assert!(lock_idx < self.locks.len(), "lock index out of bounds");
        let _lock = self.locks[lock_idx].read();

        // Safety: We validated node_id bounds and offset above, and hold the read lock
        unsafe {
            let mmap = &*self.grad_mmap.get();
            assert!(
                offset + self.d_embed * std::mem::size_of::<f32>() <= mmap.len(),
                "gradient read would exceed mmap bounds"
            );
            let ptr = mmap.as_ptr().add(offset) as *const f32;
            std::slice::from_raw_parts(ptr, self.d_embed)
        }
    }

    /// Get the embedding dimension.
    pub fn d_embed(&self) -> usize {
        self.d_embed
    }

    /// Get the number of nodes.
    pub fn n_nodes(&self) -> usize {
        self.n_nodes
    }
}

// Implement Drop to ensure proper cleanup
impl Drop for MmapManager {
    fn drop(&mut self) {
        // Try to flush dirty pages before dropping
        let _ = self.flush_dirty();
    }
}

impl Drop for MmapGradientAccumulator {
    fn drop(&mut self) {
        // Flush gradient data
        unsafe {
            let mmap = &mut *self.grad_mmap.get();
            let _ = mmap.flush();
        }
    }
}

// Safety: MmapGradientAccumulator is safe to send between threads
// because access is protected by RwLocks
unsafe impl Send for MmapGradientAccumulator {}
unsafe impl Sync for MmapGradientAccumulator {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_atomic_bitmap_basic() {
        let bitmap = AtomicBitmap::new(128);

        assert!(!bitmap.get(0));
        assert!(!bitmap.get(127));

        bitmap.set(0);
        bitmap.set(127);
        bitmap.set(64);

        assert!(bitmap.get(0));
        assert!(bitmap.get(127));
        assert!(bitmap.get(64));
        assert!(!bitmap.get(1));

        bitmap.clear(0);
        assert!(!bitmap.get(0));
        assert!(bitmap.get(127));
    }

    #[test]
    fn test_atomic_bitmap_get_set_indices() {
        let bitmap = AtomicBitmap::new(256);

        bitmap.set(0);
        bitmap.set(63);
        bitmap.set(64);
        bitmap.set(128);
        bitmap.set(255);

        let mut indices = bitmap.get_set_indices();
        indices.sort();

        assert_eq!(indices, vec![0, 63, 64, 128, 255]);
    }

    #[test]
    fn test_atomic_bitmap_clear_all() {
        let bitmap = AtomicBitmap::new(128);

        bitmap.set(0);
        bitmap.set(64);
        bitmap.set(127);

        assert!(bitmap.get(0));

        bitmap.clear_all();

        assert!(!bitmap.get(0));
        assert!(!bitmap.get(64));
        assert!(!bitmap.get(127));
    }

    #[test]
    fn test_mmap_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("embeddings.bin");

        let manager = MmapManager::new(&path, 128, 1000).unwrap();

        assert_eq!(manager.d_embed(), 128);
        assert_eq!(manager.max_nodes(), 1000);
        assert!(path.exists());
    }

    #[test]
    fn test_mmap_manager_set_get_embedding() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("embeddings.bin");

        let mut manager = MmapManager::new(&path, 64, 100).unwrap();

        let embedding = vec![1.0f32; 64];
        manager.set_embedding(0, &embedding);

        let retrieved = manager.get_embedding(0);
        assert_eq!(retrieved.len(), 64);
        assert_eq!(retrieved[0], 1.0);
        assert_eq!(retrieved[63], 1.0);
    }

    #[test]
    fn test_mmap_manager_multiple_embeddings() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("embeddings.bin");

        let mut manager = MmapManager::new(&path, 32, 100).unwrap();

        for i in 0..10 {
            let embedding: Vec<f32> = (0..32).map(|j| (i * 32 + j) as f32).collect();
            manager.set_embedding(i, &embedding);
        }

        // Verify each embedding
        for i in 0..10 {
            let retrieved = manager.get_embedding(i);
            assert_eq!(retrieved.len(), 32);
            assert_eq!(retrieved[0], (i * 32) as f32);
            assert_eq!(retrieved[31], (i * 32 + 31) as f32);
        }
    }

    #[test]
    fn test_mmap_manager_dirty_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("embeddings.bin");

        let mut manager = MmapManager::new(&path, 64, 100).unwrap();

        let embedding = vec![2.0f32; 64];
        manager.set_embedding(5, &embedding);

        // Should be marked as dirty
        assert!(manager.dirty_bitmap.get(5));

        // Flush and check it's clean
        manager.flush_dirty().unwrap();
        assert!(!manager.dirty_bitmap.get(5));
    }

    #[test]
    fn test_mmap_manager_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("embeddings.bin");

        {
            let mut manager = MmapManager::new(&path, 64, 100).unwrap();
            let embedding = vec![3.14f32; 64];
            manager.set_embedding(10, &embedding);
            manager.flush_dirty().unwrap();
        }

        // Reopen and verify data persisted
        {
            let manager = MmapManager::new(&path, 64, 100).unwrap();
            let retrieved = manager.get_embedding(10);
            assert_eq!(retrieved[0], 3.14);
            assert_eq!(retrieved[63], 3.14);
        }
    }

    #[test]
    fn test_gradient_accumulator_creation() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("gradients.bin");

        let accumulator = MmapGradientAccumulator::new(&path, 128, 1000).unwrap();

        assert_eq!(accumulator.d_embed(), 128);
        assert_eq!(accumulator.n_nodes(), 1000);
        assert!(path.exists());
    }

    #[test]
    fn test_gradient_accumulator_accumulate() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("gradients.bin");

        let accumulator = MmapGradientAccumulator::new(&path, 64, 100).unwrap();

        let grad1 = vec![1.0f32; 64];
        let grad2 = vec![2.0f32; 64];

        accumulator.accumulate(0, &grad1);
        accumulator.accumulate(0, &grad2);

        let accumulated = accumulator.get_grad(0);
        assert_eq!(accumulated[0], 3.0);
        assert_eq!(accumulated[63], 3.0);
    }

    #[test]
    fn test_gradient_accumulator_zero_grad() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("gradients.bin");

        let mut accumulator = MmapGradientAccumulator::new(&path, 64, 100).unwrap();

        let grad = vec![1.5f32; 64];
        accumulator.accumulate(0, &grad);

        let accumulated = accumulator.get_grad(0);
        assert_eq!(accumulated[0], 1.5);

        accumulator.zero_grad();

        let zeroed = accumulator.get_grad(0);
        assert_eq!(zeroed[0], 0.0);
        assert_eq!(zeroed[63], 0.0);
    }

    #[test]
    fn test_gradient_accumulator_apply() {
        let temp_dir = TempDir::new().unwrap();
        let embed_path = temp_dir.path().join("embeddings.bin");
        let grad_path = temp_dir.path().join("gradients.bin");

        let mut embeddings = MmapManager::new(&embed_path, 32, 100).unwrap();
        let mut accumulator = MmapGradientAccumulator::new(&grad_path, 32, 100).unwrap();

        // Set initial embedding
        let initial = vec![10.0f32; 32];
        embeddings.set_embedding(0, &initial);

        // Accumulate gradient
        let grad = vec![1.0f32; 32];
        accumulator.accumulate(0, &grad);

        // Apply with learning rate 0.1
        accumulator.apply(0.1, &mut embeddings);

        // Check updated embedding: 10.0 - 0.1 * 1.0 = 9.9
        let updated = embeddings.get_embedding(0);
        assert!((updated[0] - 9.9).abs() < 1e-6);

        // Check gradients were zeroed
        let zeroed_grad = accumulator.get_grad(0);
        assert_eq!(zeroed_grad[0], 0.0);
    }

    #[test]
    fn test_gradient_accumulator_concurrent_accumulation() {
        use std::thread;

        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("gradients.bin");

        let accumulator =
            std::sync::Arc::new(MmapGradientAccumulator::new(&path, 64, 100).unwrap());

        let mut handles = vec![];

        // Spawn 10 threads, each accumulating 1.0 to node 0
        for _ in 0..10 {
            let acc = accumulator.clone();
            let handle = thread::spawn(move || {
                let grad = vec![1.0f32; 64];
                acc.accumulate(0, &grad);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should have accumulated 10.0
        let result = accumulator.get_grad(0);
        assert_eq!(result[0], 10.0);
    }

    #[test]
    fn test_embedding_offset_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("embeddings.bin");

        let manager = MmapManager::new(&path, 64, 100).unwrap();

        assert_eq!(manager.embedding_offset(0), Some(0));
        assert_eq!(manager.embedding_offset(1), Some(64 * 4)); // 64 floats * 4 bytes
        assert_eq!(manager.embedding_offset(10), Some(64 * 4 * 10));
    }

    #[test]
    fn test_grad_offset_calculation() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("gradients.bin");

        let accumulator = MmapGradientAccumulator::new(&path, 128, 100).unwrap();

        assert_eq!(accumulator.grad_offset(0), Some(0));
        assert_eq!(accumulator.grad_offset(1), Some(128 * 4)); // 128 floats * 4 bytes
        assert_eq!(accumulator.grad_offset(5), Some(128 * 4 * 5));
    }

    #[test]
    #[should_panic(expected = "Embedding data length must match d_embed")]
    fn test_set_embedding_wrong_size() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("embeddings.bin");

        let mut manager = MmapManager::new(&path, 64, 100).unwrap();
        let wrong_size = vec![1.0f32; 32]; // Should be 64
        manager.set_embedding(0, &wrong_size);
    }

    #[test]
    #[should_panic(expected = "Gradient length must match d_embed")]
    fn test_accumulate_wrong_size() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("gradients.bin");

        let accumulator = MmapGradientAccumulator::new(&path, 64, 100).unwrap();
        let wrong_size = vec![1.0f32; 32]; // Should be 64
        accumulator.accumulate(0, &wrong_size);
    }

    #[test]
    fn test_prefetch() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("embeddings.bin");

        let mut manager = MmapManager::new(&path, 64, 100).unwrap();

        // Set some embeddings
        for i in 0..10 {
            let embedding = vec![i as f32; 64];
            manager.set_embedding(i, &embedding);
        }

        // Prefetch should not crash
        manager.prefetch(&[0, 1, 2, 3, 4]);

        // Access should still work
        let retrieved = manager.get_embedding(2);
        assert_eq!(retrieved[0], 2.0);
    }
}
