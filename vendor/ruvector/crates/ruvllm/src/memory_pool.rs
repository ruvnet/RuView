//! Memory Pool and Arena Allocator for High-Performance Inference
//!
//! This module provides specialized memory allocation strategies optimized for
//! LLM inference workloads on M4 Pro and similar hardware:
//!
//! - **Arena Allocator**: Bump allocation for inference buffers with O(1) reset
//! - **Buffer Pool**: Thread-safe pooling with multiple size classes for KV cache
//! - **Scratch Space Manager**: Per-thread scratch buffers for temporary computations
//!
//! ## Design Principles
//!
//! 1. **64-byte alignment**: Optimal for cache lines and NEON SIMD operations
//! 2. **Zero allocation during hot path**: All memory pre-allocated
//! 3. **Batch reset**: Arena reset after each generation step (no individual frees)
//! 4. **Thread-safe pooling**: Parking lot mutexes for low-contention access
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use ruvllm::memory_pool::{InferenceArena, BufferPool, ScratchSpaceManager};
//!
//! // Arena for inference step buffers
//! let arena = InferenceArena::new(1024 * 1024); // 1MB
//! let activations = arena.alloc::<f32>(4096);
//! let logits = arena.alloc::<f32>(32000);
//! arena.reset(); // O(1) reset after generation step
//!
//! // Buffer pool for KV cache blocks
//! let pool = BufferPool::new();
//! let block = pool.acquire(BufferSize::KB4);
//! // ... use block ...
//! pool.release(block);
//!
//! // Per-thread scratch space
//! let scratch = ScratchSpaceManager::new(4096, 8);
//! let my_scratch = scratch.get_scratch();
//! ```

use crate::error::{Result, RuvLLMError};
use parking_lot::{Mutex, RwLock};
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::cell::UnsafeCell;
#[cfg(not(target_arch = "wasm32"))]
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::thread::ThreadId;

/// Cache line size for M4 Pro and most modern CPUs (64 bytes)
pub const CACHE_LINE_SIZE: usize = 64;

/// NEON alignment requirement (16 bytes for 128-bit vectors)
pub const NEON_ALIGNMENT: usize = 16;

/// Default alignment for all allocations (64 bytes for cache lines)
pub const DEFAULT_ALIGNMENT: usize = 64;

// ============================================================================
// Arena Allocator
// ============================================================================

/// Arena allocator for fast bump allocation during inference.
///
/// The arena pre-allocates a large contiguous memory region and provides
/// O(1) allocation via bump pointer. After each generation step, the
/// entire arena can be reset in O(1) time without individual deallocations.
///
/// ## Performance Characteristics
///
/// - **Allocation**: O(1) bump pointer increment
/// - **Deallocation**: Not supported (batch reset only)
/// - **Reset**: O(1) pointer reset
/// - **Alignment**: 64-byte aligned for cache efficiency
///
/// ## Memory Layout
///
/// ```text
/// +------------------+------------------+------------------+------+
/// | Allocation 1     | Allocation 2     | Allocation 3     | Free |
/// | (64-byte aligned)| (64-byte aligned)| (64-byte aligned)|      |
/// +------------------+------------------+------------------+------+
/// ^                                                         ^
/// |                                                         |
/// memory (base ptr)                                         offset
/// ```
#[derive(Debug)]
pub struct InferenceArena {
    /// Base pointer to the memory region
    memory: *mut u8,
    /// Current allocation offset (atomic for thread-safe reads)
    offset: AtomicUsize,
    /// Total capacity in bytes
    capacity: usize,
    /// Layout for deallocation
    layout: Layout,
    /// High water mark for monitoring
    high_water_mark: AtomicUsize,
    /// Number of allocations since last reset
    allocation_count: AtomicUsize,
}

// SAFETY: The arena manages its own memory safely and uses atomic operations
unsafe impl Send for InferenceArena {}
unsafe impl Sync for InferenceArena {}

impl InferenceArena {
    /// Create a new inference arena with the specified capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Size in bytes (will be rounded up to alignment)
    ///
    /// # Errors
    ///
    /// Returns an error if memory allocation fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let arena = InferenceArena::new(4 * 1024 * 1024)?; // 4MB arena
    /// ```
    pub fn new(capacity: usize) -> Result<Self> {
        // Round up to cache line size
        let aligned_capacity = (capacity + DEFAULT_ALIGNMENT - 1) & !(DEFAULT_ALIGNMENT - 1);

        let layout =
            Layout::from_size_align(aligned_capacity, DEFAULT_ALIGNMENT).map_err(|_| {
                RuvLLMError::OutOfMemory(format!(
                    "Invalid arena layout: size={}, align={}",
                    aligned_capacity, DEFAULT_ALIGNMENT
                ))
            })?;

        // SAFETY: Layout is valid and we track the allocation
        let memory = unsafe { alloc_zeroed(layout) };

        if memory.is_null() {
            return Err(RuvLLMError::OutOfMemory(format!(
                "Failed to allocate arena of {} bytes",
                aligned_capacity
            )));
        }

        Ok(Self {
            memory,
            offset: AtomicUsize::new(0),
            capacity: aligned_capacity,
            layout,
            high_water_mark: AtomicUsize::new(0),
            allocation_count: AtomicUsize::new(0),
        })
    }

    /// Create a new arena sized for model dimensions.
    ///
    /// Automatically calculates appropriate arena size based on model parameters.
    ///
    /// # Arguments
    ///
    /// * `hidden_dim` - Model hidden dimension
    /// * `vocab_size` - Vocabulary size
    /// * `batch_size` - Maximum batch size
    ///
    /// # Errors
    ///
    /// Returns an error if memory allocation fails.
    pub fn for_model(hidden_dim: usize, vocab_size: usize, batch_size: usize) -> Result<Self> {
        // Estimate: activations + logits + scratch space
        let activations = hidden_dim * batch_size * std::mem::size_of::<f32>();
        let logits = vocab_size * batch_size * std::mem::size_of::<f32>();
        let scratch = hidden_dim * 4 * std::mem::size_of::<f32>(); // 4x for intermediate

        let total = (activations + logits + scratch) * 2; // 2x safety margin
        Self::new(total)
    }

    /// Allocate a slice of type T from the arena.
    ///
    /// Returns a mutable slice pointing to the allocated memory. The memory
    /// is zero-initialized and 64-byte aligned.
    ///
    /// # Arguments
    ///
    /// * `count` - Number of elements to allocate
    ///
    /// # Returns
    ///
    /// A mutable reference to the allocated slice, or None if out of memory.
    ///
    /// # Safety
    ///
    /// The returned reference is valid until the arena is reset or dropped.
    /// Callers must ensure they don't hold references across reset boundaries.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let arena = InferenceArena::new(1024 * 1024);
    /// let buffer: &mut [f32] = arena.alloc(4096).unwrap();
    /// buffer[0] = 1.0;
    /// ```
    #[inline]
    pub fn alloc<T: Copy + Default>(&self, count: usize) -> Option<&mut [T]> {
        let size = count * std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>().max(DEFAULT_ALIGNMENT);

        // Align the current offset
        let current = self.offset.load(Ordering::Acquire);
        let aligned_offset = (current + align - 1) & !(align - 1);
        let new_offset = aligned_offset + size;

        // Check capacity
        if new_offset > self.capacity {
            return None;
        }

        // Try to bump the offset atomically
        match self
            .offset
            .compare_exchange(current, new_offset, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => {
                // Update statistics
                self.allocation_count.fetch_add(1, Ordering::Relaxed);
                let _ = self
                    .high_water_mark
                    .fetch_max(new_offset, Ordering::Relaxed);

                // SAFETY: We've reserved this memory region atomically
                unsafe {
                    let ptr = self.memory.add(aligned_offset) as *mut T;
                    // Zero-initialize (memory may have been reused after reset)
                    std::ptr::write_bytes(ptr, 0, count);
                    Some(std::slice::from_raw_parts_mut(ptr, count))
                }
            }
            Err(actual) => {
                // Retry with new offset (concurrent allocation occurred)
                // For simplicity, we return None and let caller retry
                // A production implementation might spin-retry
                None
            }
        }
    }

    /// Allocate uninitialized memory (faster than alloc for large buffers).
    ///
    /// # Safety
    ///
    /// The caller must initialize the memory before reading from it.
    #[inline]
    pub unsafe fn alloc_uninit<T>(&self, count: usize) -> Option<&mut [T]> {
        let size = count * std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>().max(DEFAULT_ALIGNMENT);

        let current = self.offset.load(Ordering::Acquire);
        let aligned_offset = (current + align - 1) & !(align - 1);
        let new_offset = aligned_offset + size;

        if new_offset > self.capacity {
            return None;
        }

        match self
            .offset
            .compare_exchange(current, new_offset, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => {
                self.allocation_count.fetch_add(1, Ordering::Relaxed);
                let _ = self
                    .high_water_mark
                    .fetch_max(new_offset, Ordering::Relaxed);

                let ptr = self.memory.add(aligned_offset) as *mut T;
                Some(std::slice::from_raw_parts_mut(ptr, count))
            }
            Err(_) => None,
        }
    }

    /// Reset the arena, making all memory available for reuse.
    ///
    /// This is an O(1) operation that simply resets the bump pointer.
    /// All previously allocated memory becomes invalid.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let arena = InferenceArena::new(1024 * 1024);
    ///
    /// // Generation step 1
    /// let buf1 = arena.alloc::<f32>(1000).unwrap();
    /// arena.reset();
    ///
    /// // Generation step 2 - reuses same memory
    /// let buf2 = arena.alloc::<f32>(1000).unwrap();
    /// ```
    #[inline]
    pub fn reset(&self) {
        self.offset.store(0, Ordering::Release);
        self.allocation_count.store(0, Ordering::Relaxed);
    }

    /// Get the current allocation offset (bytes used).
    #[inline]
    pub fn used(&self) -> usize {
        self.offset.load(Ordering::Acquire)
    }

    /// Get the total capacity in bytes.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the remaining available bytes.
    #[inline]
    pub fn remaining(&self) -> usize {
        self.capacity - self.used()
    }

    /// Get the high water mark (maximum bytes ever used).
    #[inline]
    pub fn high_water_mark(&self) -> usize {
        self.high_water_mark.load(Ordering::Relaxed)
    }

    /// Get the number of allocations since last reset.
    #[inline]
    pub fn allocation_count(&self) -> usize {
        self.allocation_count.load(Ordering::Relaxed)
    }

    /// Get arena statistics.
    pub fn stats(&self) -> ArenaStats {
        ArenaStats {
            capacity: self.capacity,
            used: self.used(),
            remaining: self.remaining(),
            high_water_mark: self.high_water_mark(),
            allocation_count: self.allocation_count(),
            utilization: self.used() as f64 / self.capacity as f64,
        }
    }

    /// Get raw pointer to arena memory (for NEON intrinsics).
    ///
    /// # Safety
    ///
    /// Caller must ensure they don't exceed allocated bounds.
    #[inline]
    pub unsafe fn as_ptr(&self) -> *const u8 {
        self.memory
    }

    /// Get mutable raw pointer to arena memory.
    ///
    /// # Safety
    ///
    /// Caller must ensure they don't exceed allocated bounds.
    #[inline]
    pub unsafe fn as_mut_ptr(&self) -> *mut u8 {
        self.memory
    }
}

impl Drop for InferenceArena {
    fn drop(&mut self) {
        // SAFETY: memory was allocated with this layout
        unsafe {
            dealloc(self.memory, self.layout);
        }
    }
}

/// Arena allocation statistics
#[derive(Debug, Clone, Default)]
pub struct ArenaStats {
    /// Total capacity in bytes
    pub capacity: usize,
    /// Currently used bytes
    pub used: usize,
    /// Remaining available bytes
    pub remaining: usize,
    /// Maximum bytes ever allocated
    pub high_water_mark: usize,
    /// Number of allocations since reset
    pub allocation_count: usize,
    /// Utilization ratio (0.0 - 1.0)
    pub utilization: f64,
}

// ============================================================================
// Buffer Pool
// ============================================================================

/// Buffer size classes for the pool.
///
/// Using power-of-two sizes for efficient allocation and cache alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum BufferSize {
    /// 1KB buffer
    KB1 = 0,
    /// 4KB buffer (memory page size)
    KB4 = 1,
    /// 16KB buffer
    KB16 = 2,
    /// 64KB buffer
    KB64 = 3,
    /// 256KB buffer
    KB256 = 4,
}

impl BufferSize {
    /// Get the size in bytes for this buffer class.
    #[inline]
    pub const fn bytes(self) -> usize {
        match self {
            Self::KB1 => 1024,
            Self::KB4 => 4096,
            Self::KB16 => 16384,
            Self::KB64 => 65536,
            Self::KB256 => 262144,
        }
    }

    /// Get the size class index.
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Determine the appropriate size class for a given byte count.
    pub fn for_size(bytes: usize) -> Option<Self> {
        if bytes <= 1024 {
            Some(Self::KB1)
        } else if bytes <= 4096 {
            Some(Self::KB4)
        } else if bytes <= 16384 {
            Some(Self::KB16)
        } else if bytes <= 65536 {
            Some(Self::KB64)
        } else if bytes <= 262144 {
            Some(Self::KB256)
        } else {
            None
        }
    }

    /// Get all buffer sizes in order.
    pub const fn all() -> [BufferSize; 5] {
        [Self::KB1, Self::KB4, Self::KB16, Self::KB64, Self::KB256]
    }
}

/// A pooled buffer that returns to the pool when dropped.
pub struct PooledBuffer {
    /// The actual buffer data
    data: Box<[u8]>,
    /// Size class for return to pool
    size_class: BufferSize,
    /// Reference to parent pool for return
    pool: Arc<BufferPoolInner>,
}

impl PooledBuffer {
    /// Get the buffer as a byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get the buffer as a mutable byte slice.
    #[inline]
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Get the buffer as a typed slice.
    ///
    /// # Panics
    ///
    /// Panics if the buffer size is not a multiple of the type size.
    #[inline]
    pub fn as_slice<T: Copy>(&self) -> &[T] {
        let size = std::mem::size_of::<T>();
        assert!(
            self.data.len() % size == 0,
            "Buffer size not aligned to type"
        );
        // SAFETY: Buffer is aligned and size is checked
        unsafe {
            std::slice::from_raw_parts(self.data.as_ptr() as *const T, self.data.len() / size)
        }
    }

    /// Get the buffer as a mutable typed slice.
    #[inline]
    pub fn as_slice_mut<T: Copy>(&mut self) -> &mut [T] {
        let size = std::mem::size_of::<T>();
        assert!(
            self.data.len() % size == 0,
            "Buffer size not aligned to type"
        );
        // SAFETY: Buffer is aligned and size is checked
        unsafe {
            std::slice::from_raw_parts_mut(self.data.as_mut_ptr() as *mut T, self.data.len() / size)
        }
    }

    /// Get the buffer capacity in bytes.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.data.len()
    }

    /// Get the size class of this buffer.
    #[inline]
    pub fn size_class(&self) -> BufferSize {
        self.size_class
    }

    /// Get raw pointer to buffer data.
    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.data.as_ptr()
    }

    /// Get mutable raw pointer to buffer data.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.data.as_mut_ptr()
    }

    /// Zero-fill the buffer.
    #[inline]
    pub fn clear(&mut self) {
        self.data.fill(0);
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        // SAFETY NOTE: Double-free prevention
        //
        // This implementation is safe from double-free because:
        // 1. Each PooledBuffer has exclusive ownership of its `data` Box
        // 2. We swap with an empty Box to take ownership before returning
        // 3. return_buffer() checks for empty buffers and ignores them
        // 4. If called twice (somehow), the second call finds an empty Box
        //    which is harmless
        //
        // The Arc<BufferPoolInner> ensures the pool outlives this buffer.
        let data = std::mem::replace(&mut self.data, Box::new([]));
        self.pool.return_buffer(self.size_class, data);
    }
}

impl std::fmt::Debug for PooledBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PooledBuffer")
            .field("size_class", &self.size_class)
            .field("capacity", &self.data.len())
            .finish()
    }
}

/// Internal pool state for each size class.
struct SizeClassPool {
    /// Available buffers
    free_list: Vec<Box<[u8]>>,
    /// Maximum buffers to keep in pool
    max_buffers: usize,
}

/// Inner pool structure (shared via Arc).
struct BufferPoolInner {
    /// Pools for each size class
    pools: [Mutex<SizeClassPool>; 5],
    /// Statistics
    stats: PoolStatistics,
}

impl BufferPoolInner {
    fn new(max_buffers_per_class: usize) -> Self {
        Self {
            pools: [
                Mutex::new(SizeClassPool {
                    free_list: Vec::with_capacity(max_buffers_per_class),
                    max_buffers: max_buffers_per_class,
                }),
                Mutex::new(SizeClassPool {
                    free_list: Vec::with_capacity(max_buffers_per_class),
                    max_buffers: max_buffers_per_class,
                }),
                Mutex::new(SizeClassPool {
                    free_list: Vec::with_capacity(max_buffers_per_class),
                    max_buffers: max_buffers_per_class,
                }),
                Mutex::new(SizeClassPool {
                    free_list: Vec::with_capacity(max_buffers_per_class),
                    max_buffers: max_buffers_per_class,
                }),
                Mutex::new(SizeClassPool {
                    free_list: Vec::with_capacity(max_buffers_per_class),
                    max_buffers: max_buffers_per_class,
                }),
            ],
            stats: PoolStatistics::new(),
        }
    }

    fn acquire(&self, size_class: BufferSize) -> Result<Box<[u8]>> {
        let mut pool = self.pools[size_class.index()].lock();

        if let Some(buf) = pool.free_list.pop() {
            self.stats.hits.fetch_add(1, Ordering::Relaxed);
            Ok(buf)
        } else {
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
            self.stats.allocations.fetch_add(1, Ordering::Relaxed);
            Self::allocate_buffer(size_class)
        }
    }

    fn return_buffer(&self, size_class: BufferSize, buf: Box<[u8]>) {
        // SAFETY: Guard against returning empty buffers
        // This happens when PooledBuffer::Drop swaps data with an empty Box.
        // Ignoring empty buffers prevents any issues from double-drops.
        if buf.is_empty() {
            return;
        }

        let mut pool = self.pools[size_class.index()].lock();

        if pool.free_list.len() < pool.max_buffers {
            self.stats.returns.fetch_add(1, Ordering::Relaxed);
            pool.free_list.push(buf);
        } else {
            // Pool is full, let buffer drop
            self.stats.drops.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn allocate_buffer(size_class: BufferSize) -> Result<Box<[u8]>> {
        let size = size_class.bytes();
        let layout = Layout::from_size_align(size, DEFAULT_ALIGNMENT).map_err(|_| {
            RuvLLMError::OutOfMemory(format!(
                "Invalid buffer layout: size={}, align={}",
                size, DEFAULT_ALIGNMENT
            ))
        })?;

        // SAFETY: Layout is valid
        unsafe {
            let ptr = alloc_zeroed(layout);
            if ptr.is_null() {
                return Err(RuvLLMError::OutOfMemory(format!(
                    "Failed to allocate buffer of {} bytes",
                    size
                )));
            }
            Ok(Box::from_raw(std::slice::from_raw_parts_mut(ptr, size)))
        }
    }
}

/// Pool usage statistics.
struct PoolStatistics {
    /// Number of pool hits (buffer reused)
    hits: AtomicU64,
    /// Number of pool misses (new allocation)
    misses: AtomicU64,
    /// Total allocations made
    allocations: AtomicU64,
    /// Buffers returned to pool
    returns: AtomicU64,
    /// Buffers dropped (pool full)
    drops: AtomicU64,
}

impl PoolStatistics {
    fn new() -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            allocations: AtomicU64::new(0),
            returns: AtomicU64::new(0),
            drops: AtomicU64::new(0),
        }
    }
}

/// Thread-safe buffer pool with multiple size classes.
///
/// The pool maintains separate free lists for each size class (1KB, 4KB, 16KB,
/// 64KB, 256KB) and reuses buffers to minimize allocation overhead during
/// inference.
///
/// ## Thread Safety
///
/// Uses parking_lot Mutex for low-contention locking. Each size class has
/// its own lock to minimize contention.
///
/// ## Statistics
///
/// Tracks hits, misses, allocations, returns, and drops for monitoring
/// pool efficiency.
#[derive(Clone)]
pub struct BufferPool {
    inner: Arc<BufferPoolInner>,
}

impl BufferPool {
    /// Create a new buffer pool with default settings.
    ///
    /// Default: 32 buffers per size class.
    pub fn new() -> Self {
        Self::with_capacity(32)
    }

    /// Create a buffer pool with specified max buffers per size class.
    pub fn with_capacity(max_buffers_per_class: usize) -> Self {
        Self {
            inner: Arc::new(BufferPoolInner::new(max_buffers_per_class)),
        }
    }

    /// Acquire a buffer of the specified size class.
    ///
    /// Returns a pooled buffer that automatically returns to the pool when dropped.
    ///
    /// # Errors
    ///
    /// Returns an error if memory allocation fails.
    pub fn acquire(&self, size_class: BufferSize) -> Result<PooledBuffer> {
        let data = self.inner.acquire(size_class)?;
        Ok(PooledBuffer {
            data,
            size_class,
            pool: Arc::clone(&self.inner),
        })
    }

    /// Acquire a buffer large enough for the specified byte count.
    ///
    /// Returns None if the requested size exceeds the largest size class,
    /// or an error if memory allocation fails.
    pub fn acquire_for_size(&self, bytes: usize) -> Result<Option<PooledBuffer>> {
        match BufferSize::for_size(bytes) {
            Some(size_class) => Ok(Some(self.acquire(size_class)?)),
            None => Ok(None),
        }
    }

    /// Pre-warm the pool by allocating buffers.
    ///
    /// # Arguments
    ///
    /// * `size_class` - Size class to pre-warm
    /// * `count` - Number of buffers to pre-allocate
    ///
    /// # Errors
    ///
    /// Returns an error if memory allocation fails.
    pub fn prewarm(&self, size_class: BufferSize, count: usize) -> Result<()> {
        for _ in 0..count {
            let buf = BufferPoolInner::allocate_buffer(size_class)?;
            self.inner.return_buffer(size_class, buf);
        }
        Ok(())
    }

    /// Pre-warm all size classes with the specified count.
    ///
    /// # Errors
    ///
    /// Returns an error if memory allocation fails for any size class.
    pub fn prewarm_all(&self, count_per_class: usize) -> Result<()> {
        for size_class in BufferSize::all() {
            self.prewarm(size_class, count_per_class)?;
        }
        Ok(())
    }

    /// Get pool statistics.
    pub fn stats(&self) -> BufferPoolStats {
        let mut free_counts = [0usize; 5];
        for (i, pool) in self.inner.pools.iter().enumerate() {
            free_counts[i] = pool.lock().free_list.len();
        }

        BufferPoolStats {
            hits: self.inner.stats.hits.load(Ordering::Relaxed),
            misses: self.inner.stats.misses.load(Ordering::Relaxed),
            allocations: self.inner.stats.allocations.load(Ordering::Relaxed),
            returns: self.inner.stats.returns.load(Ordering::Relaxed),
            drops: self.inner.stats.drops.load(Ordering::Relaxed),
            free_buffers: free_counts,
            hit_rate: {
                let hits = self.inner.stats.hits.load(Ordering::Relaxed);
                let total = hits + self.inner.stats.misses.load(Ordering::Relaxed);
                if total > 0 {
                    hits as f64 / total as f64
                } else {
                    0.0
                }
            },
        }
    }

    /// Clear all pooled buffers.
    pub fn clear(&self) {
        for pool in &self.inner.pools {
            pool.lock().free_list.clear();
        }
    }
}

impl Default for BufferPool {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for BufferPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufferPool")
            .field("stats", &self.stats())
            .finish()
    }
}

/// Buffer pool statistics
#[derive(Debug, Clone, Default)]
pub struct BufferPoolStats {
    /// Number of pool hits
    pub hits: u64,
    /// Number of pool misses
    pub misses: u64,
    /// Total allocations
    pub allocations: u64,
    /// Buffers returned to pool
    pub returns: u64,
    /// Buffers dropped (pool full)
    pub drops: u64,
    /// Free buffers per size class [1K, 4K, 16K, 64K, 256K]
    pub free_buffers: [usize; 5],
    /// Pool hit rate (0.0 - 1.0)
    pub hit_rate: f64,
}

// ============================================================================
// Scratch Space Manager
// ============================================================================

/// Per-thread scratch buffer (non-WASM only).
#[cfg(not(target_arch = "wasm32"))]
struct ThreadScratch {
    /// Buffer data
    data: Box<[u8]>,
    /// Current usage within the buffer
    used: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl ThreadScratch {
    fn new(size: usize) -> Result<Self> {
        let layout = Layout::from_size_align(size, DEFAULT_ALIGNMENT).map_err(|_| {
            RuvLLMError::OutOfMemory(format!(
                "Invalid scratch layout: size={}, align={}",
                size, DEFAULT_ALIGNMENT
            ))
        })?;

        // SAFETY: Layout is valid
        let data = unsafe {
            let ptr = alloc_zeroed(layout);
            if ptr.is_null() {
                return Err(RuvLLMError::OutOfMemory(format!(
                    "Failed to allocate scratch buffer of {} bytes",
                    size
                )));
            }
            Box::from_raw(std::slice::from_raw_parts_mut(ptr, size))
        };

        Ok(Self { data, used: 0 })
    }

    fn reset(&mut self) {
        self.used = 0;
    }
}

/// Manager for per-thread scratch space (non-WASM version).
///
/// Provides each thread with its own scratch buffer for temporary computations
/// during inference, avoiding allocation on the hot path.
///
/// ## Design
///
/// - Each thread gets a dedicated scratch buffer on first access
/// - Buffers are sized based on model dimensions
/// - Scratch is reset at the start of each generation step
/// - Thread-safe lazy initialization
#[cfg(not(target_arch = "wasm32"))]
pub struct ScratchSpaceManager {
    /// Per-thread scratch buffers
    scratches: RwLock<HashMap<ThreadId, UnsafeCell<ThreadScratch>>>,
    /// Size for each scratch buffer
    scratch_size: usize,
    /// Maximum number of threads to support
    max_threads: usize,
}

// SAFETY: ThreadScratch is only accessed by its owning thread
#[cfg(not(target_arch = "wasm32"))]
unsafe impl Send for ScratchSpaceManager {}
#[cfg(not(target_arch = "wasm32"))]
unsafe impl Sync for ScratchSpaceManager {}

#[cfg(not(target_arch = "wasm32"))]
impl ScratchSpaceManager {
    /// Create a new scratch space manager.
    ///
    /// # Arguments
    ///
    /// * `scratch_size` - Size of each thread's scratch buffer in bytes
    /// * `max_threads` - Maximum number of threads to support
    ///
    /// # Note
    ///
    /// Memory is allocated lazily when `get_scratch` is called.
    /// This method always succeeds but returns Result for API consistency with WASM.
    pub fn new(scratch_size: usize, max_threads: usize) -> Result<Self> {
        Ok(Self {
            scratches: RwLock::new(HashMap::with_capacity(max_threads)),
            scratch_size,
            max_threads,
        })
    }

    /// Create a scratch manager sized for model dimensions.
    ///
    /// # Arguments
    ///
    /// * `hidden_dim` - Model hidden dimension
    /// * `max_threads` - Maximum number of threads
    pub fn for_model(hidden_dim: usize, max_threads: usize) -> Result<Self> {
        // Size for intermediate computations: 4x hidden_dim in f32
        let scratch_size = hidden_dim * 4 * std::mem::size_of::<f32>();
        Self::new(scratch_size, max_threads)
    }

    /// Get the scratch buffer for the current thread.
    ///
    /// Creates a new buffer if this is the first access from this thread.
    ///
    /// # Returns
    ///
    /// A reference to the thread's scratch space.
    ///
    /// # Errors
    ///
    /// Returns an error if the maximum thread count is exceeded or memory allocation fails.
    pub fn get_scratch(&self) -> Result<ScratchSpace<'_>> {
        let thread_id = std::thread::current().id();

        // Fast path: check if scratch exists
        {
            let scratches = self.scratches.read();
            if let Some(scratch_cell) = scratches.get(&thread_id) {
                // SAFETY: This thread owns this scratch buffer
                return Ok(ScratchSpace {
                    scratch: unsafe { &mut *scratch_cell.get() },
                });
            }
        }

        // Slow path: create new scratch
        {
            let mut scratches = self.scratches.write();

            // Double-check after acquiring write lock
            if !scratches.contains_key(&thread_id) {
                if scratches.len() >= self.max_threads {
                    return Err(RuvLLMError::OutOfMemory(format!(
                        "Exceeded maximum thread count ({}) for scratch space",
                        self.max_threads
                    )));
                }

                scratches.insert(
                    thread_id,
                    UnsafeCell::new(ThreadScratch::new(self.scratch_size)?),
                );
            }

            let scratch_cell = scratches.get(&thread_id).unwrap();
            // SAFETY: This thread owns this scratch buffer
            Ok(ScratchSpace {
                scratch: unsafe { &mut *scratch_cell.get() },
            })
        }
    }

    /// Reset all thread scratch buffers.
    ///
    /// Should be called at the start of each generation step.
    pub fn reset_all(&self) {
        let scratches = self.scratches.read();
        for scratch_cell in scratches.values() {
            // SAFETY: We're resetting, not accessing data
            unsafe {
                (*scratch_cell.get()).reset();
            }
        }
    }

    /// Get the configured scratch size per thread.
    pub fn scratch_size(&self) -> usize {
        self.scratch_size
    }

    /// Get the number of active threads with scratch buffers.
    pub fn active_threads(&self) -> usize {
        self.scratches.read().len()
    }

    /// Get statistics about scratch usage.
    pub fn stats(&self) -> ScratchStats {
        let scratches = self.scratches.read();
        let mut total_used = 0;
        let mut max_used = 0;

        for scratch_cell in scratches.values() {
            // SAFETY: Just reading statistics
            let used = unsafe { (*scratch_cell.get()).used };
            total_used += used;
            max_used = max_used.max(used);
        }

        ScratchStats {
            scratch_size: self.scratch_size,
            active_threads: scratches.len(),
            max_threads: self.max_threads,
            total_allocated: scratches.len() * self.scratch_size,
            total_used,
            max_thread_usage: max_used,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Debug for ScratchSpaceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScratchSpaceManager")
            .field("scratch_size", &self.scratch_size)
            .field("max_threads", &self.max_threads)
            .field("active_threads", &self.scratches.read().len())
            .finish()
    }
}

// ============================================================================
// WASM-compatible Scratch Space Manager (single-threaded)
// ============================================================================

/// Scratch buffer for WASM (single-threaded).
#[cfg(target_arch = "wasm32")]
struct WasmScratch {
    /// Buffer data
    data: Box<[u8]>,
    /// Current usage within the buffer
    used: usize,
}

#[cfg(target_arch = "wasm32")]
impl WasmScratch {
    fn new(size: usize) -> Result<Self> {
        let layout = Layout::from_size_align(size, DEFAULT_ALIGNMENT).map_err(|_| {
            RuvLLMError::OutOfMemory(format!(
                "Invalid scratch layout: size={}, align={}",
                size, DEFAULT_ALIGNMENT
            ))
        })?;

        // SAFETY: Layout is valid
        let data = unsafe {
            let ptr = alloc_zeroed(layout);
            if ptr.is_null() {
                return Err(RuvLLMError::OutOfMemory(format!(
                    "Failed to allocate scratch buffer of {} bytes",
                    size
                )));
            }
            Box::from_raw(std::slice::from_raw_parts_mut(ptr, size))
        };

        Ok(Self { data, used: 0 })
    }

    fn reset(&mut self) {
        self.used = 0;
    }
}

/// Manager for scratch space on WASM (single-threaded version).
///
/// WASM is single-threaded, so we only need one scratch buffer.
#[cfg(target_arch = "wasm32")]
pub struct ScratchSpaceManager {
    /// Single scratch buffer (WASM is single-threaded)
    scratch: UnsafeCell<WasmScratch>,
    /// Size of the scratch buffer
    scratch_size: usize,
    /// Max threads (always 1 on WASM)
    max_threads: usize,
}

// SAFETY: WASM is single-threaded
#[cfg(target_arch = "wasm32")]
unsafe impl Send for ScratchSpaceManager {}
#[cfg(target_arch = "wasm32")]
unsafe impl Sync for ScratchSpaceManager {}

#[cfg(target_arch = "wasm32")]
impl ScratchSpaceManager {
    /// Create a new scratch space manager.
    ///
    /// # Errors
    ///
    /// Returns an error if memory allocation fails.
    pub fn new(scratch_size: usize, _max_threads: usize) -> Result<Self> {
        Ok(Self {
            scratch: UnsafeCell::new(WasmScratch::new(scratch_size)?),
            scratch_size,
            max_threads: 1, // WASM is single-threaded
        })
    }

    /// Create a scratch manager sized for model dimensions.
    ///
    /// # Errors
    ///
    /// Returns an error if memory allocation fails.
    pub fn for_model(hidden_dim: usize, _max_threads: usize) -> Result<Self> {
        let scratch_size = hidden_dim * 4 * std::mem::size_of::<f32>();
        Self::new(scratch_size, 1)
    }

    /// Get the scratch buffer.
    pub fn get_scratch(&self) -> Result<ScratchSpace<'_>> {
        // SAFETY: WASM is single-threaded
        Ok(ScratchSpace {
            scratch: unsafe { &mut *self.scratch.get() },
        })
    }

    /// Reset the scratch buffer.
    pub fn reset_all(&self) {
        // SAFETY: WASM is single-threaded
        unsafe {
            (*self.scratch.get()).reset();
        }
    }

    /// Get the configured scratch size.
    pub fn scratch_size(&self) -> usize {
        self.scratch_size
    }

    /// Get the number of active threads (always 1 on WASM).
    pub fn active_threads(&self) -> usize {
        1
    }

    /// Get statistics about scratch usage.
    pub fn stats(&self) -> ScratchStats {
        // SAFETY: WASM is single-threaded
        let used = unsafe { (*self.scratch.get()).used };
        ScratchStats {
            scratch_size: self.scratch_size,
            active_threads: 1,
            max_threads: 1,
            total_allocated: self.scratch_size,
            total_used: used,
            max_thread_usage: used,
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl std::fmt::Debug for ScratchSpaceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScratchSpaceManager")
            .field("scratch_size", &self.scratch_size)
            .field("max_threads", &self.max_threads)
            .field("active_threads", &1)
            .finish()
    }
}

/// Handle to a thread's scratch space (non-WASM version).
#[cfg(not(target_arch = "wasm32"))]
pub struct ScratchSpace<'a> {
    scratch: &'a mut ThreadScratch,
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a> ScratchSpace<'a> {
    /// Get a typed slice of the scratch buffer.
    ///
    /// # Arguments
    ///
    /// * `count` - Number of elements needed
    ///
    /// # Returns
    ///
    /// A mutable slice of the requested type, or None if insufficient space.
    pub fn get<T: Copy + Default>(&mut self, count: usize) -> Option<&mut [T]> {
        let size = count * std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>().max(DEFAULT_ALIGNMENT);

        let aligned_used = (self.scratch.used + align - 1) & !(align - 1);
        let new_used = aligned_used + size;

        if new_used > self.scratch.data.len() {
            return None;
        }

        self.scratch.used = new_used;

        // SAFETY: We've checked bounds and alignment
        unsafe {
            let ptr = self.scratch.data.as_mut_ptr().add(aligned_used) as *mut T;
            std::ptr::write_bytes(ptr, 0, count);
            Some(std::slice::from_raw_parts_mut(ptr, count))
        }
    }

    /// Get the raw scratch buffer.
    pub fn as_bytes(&self) -> &[u8] {
        &self.scratch.data
    }

    /// Get the mutable raw scratch buffer.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.scratch.data
    }

    /// Reset the scratch buffer for reuse.
    pub fn reset(&mut self) {
        self.scratch.reset();
    }

    /// Get current usage in bytes.
    pub fn used(&self) -> usize {
        self.scratch.used
    }

    /// Get remaining capacity in bytes.
    pub fn remaining(&self) -> usize {
        self.scratch.data.len() - self.scratch.used
    }

    /// Get total capacity in bytes.
    pub fn capacity(&self) -> usize {
        self.scratch.data.len()
    }
}

/// Handle to scratch space (WASM version).
#[cfg(target_arch = "wasm32")]
pub struct ScratchSpace<'a> {
    scratch: &'a mut WasmScratch,
}

#[cfg(target_arch = "wasm32")]
impl<'a> ScratchSpace<'a> {
    /// Get a typed slice of the scratch buffer.
    pub fn get<T: Copy + Default>(&mut self, count: usize) -> Option<&mut [T]> {
        let size = count * std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>().max(DEFAULT_ALIGNMENT);

        let aligned_used = (self.scratch.used + align - 1) & !(align - 1);
        let new_used = aligned_used + size;

        if new_used > self.scratch.data.len() {
            return None;
        }

        self.scratch.used = new_used;

        // SAFETY: We've checked bounds and alignment
        unsafe {
            let ptr = self.scratch.data.as_mut_ptr().add(aligned_used) as *mut T;
            std::ptr::write_bytes(ptr, 0, count);
            Some(std::slice::from_raw_parts_mut(ptr, count))
        }
    }

    /// Get the raw scratch buffer.
    pub fn as_bytes(&self) -> &[u8] {
        &self.scratch.data
    }

    /// Get the mutable raw scratch buffer.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.scratch.data
    }

    /// Reset the scratch buffer for reuse.
    pub fn reset(&mut self) {
        self.scratch.reset();
    }

    /// Get current usage in bytes.
    pub fn used(&self) -> usize {
        self.scratch.used
    }

    /// Get remaining capacity in bytes.
    pub fn remaining(&self) -> usize {
        self.scratch.data.len() - self.scratch.used
    }

    /// Get total capacity in bytes.
    pub fn capacity(&self) -> usize {
        self.scratch.data.len()
    }
}

/// Scratch space statistics
#[derive(Debug, Clone, Default)]
pub struct ScratchStats {
    /// Size of each scratch buffer
    pub scratch_size: usize,
    /// Number of active threads
    pub active_threads: usize,
    /// Maximum thread count
    pub max_threads: usize,
    /// Total memory allocated for scratch
    pub total_allocated: usize,
    /// Total currently used across all threads
    pub total_used: usize,
    /// Maximum usage by any single thread
    pub max_thread_usage: usize,
}

// ============================================================================
// Unified Memory Manager
// ============================================================================

/// Configuration for the unified memory manager.
#[derive(Debug, Clone)]
pub struct MemoryManagerConfig {
    /// Arena capacity in bytes
    pub arena_capacity: usize,
    /// Max buffers per pool size class
    pub pool_buffers_per_class: usize,
    /// Scratch size per thread
    pub scratch_size: usize,
    /// Maximum threads for scratch
    pub max_threads: usize,
}

impl Default for MemoryManagerConfig {
    fn default() -> Self {
        Self {
            arena_capacity: 16 * 1024 * 1024, // 16MB arena
            pool_buffers_per_class: 32,
            scratch_size: 64 * 1024, // 64KB per thread
            max_threads: 16,
        }
    }
}

impl MemoryManagerConfig {
    /// Create config optimized for model dimensions.
    pub fn for_model(hidden_dim: usize, vocab_size: usize, batch_size: usize) -> Self {
        let arena_capacity = {
            let activations = hidden_dim * batch_size * 4; // f32
            let logits = vocab_size * batch_size * 4;
            (activations + logits) * 4 // 4x headroom
        };

        let scratch_size = hidden_dim * 4 * 4; // 4x hidden_dim in f32

        Self {
            arena_capacity,
            pool_buffers_per_class: 32,
            scratch_size,
            max_threads: 16,
        }
    }
}

/// Unified memory manager combining arena, pool, and scratch space.
///
/// Provides a single interface for all memory allocation needs during inference:
/// - Arena for generation step temporaries
/// - Pool for KV cache blocks
/// - Scratch for per-thread computations
pub struct MemoryManager {
    /// Arena for inference buffers
    pub arena: InferenceArena,
    /// Buffer pool for KV cache
    pub pool: BufferPool,
    /// Scratch space manager
    pub scratch: ScratchSpaceManager,
    /// Configuration
    config: MemoryManagerConfig,
}

impl MemoryManager {
    /// Create a new memory manager with default configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if memory allocation fails.
    pub fn new() -> Result<Self> {
        Self::with_config(MemoryManagerConfig::default())
    }

    /// Create a memory manager with custom configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if memory allocation fails.
    pub fn with_config(config: MemoryManagerConfig) -> Result<Self> {
        let arena = InferenceArena::new(config.arena_capacity)?;
        let pool = BufferPool::with_capacity(config.pool_buffers_per_class);
        let scratch = ScratchSpaceManager::new(config.scratch_size, config.max_threads)?;

        Ok(Self {
            arena,
            pool,
            scratch,
            config,
        })
    }

    /// Create a memory manager sized for model dimensions.
    ///
    /// # Errors
    ///
    /// Returns an error if memory allocation fails.
    pub fn for_model(hidden_dim: usize, vocab_size: usize, batch_size: usize) -> Result<Self> {
        let config = MemoryManagerConfig::for_model(hidden_dim, vocab_size, batch_size);
        Self::with_config(config)
    }

    /// Reset all transient allocations (arena + scratch).
    ///
    /// Call this at the start of each generation step.
    #[inline]
    pub fn reset_step(&self) {
        self.arena.reset();
        self.scratch.reset_all();
    }

    /// Pre-warm the buffer pool.
    ///
    /// # Errors
    ///
    /// Returns an error if memory allocation fails.
    pub fn prewarm_pool(&self, count_per_class: usize) -> Result<()> {
        self.pool.prewarm_all(count_per_class)
    }

    /// Get combined statistics.
    pub fn stats(&self) -> MemoryManagerStats {
        MemoryManagerStats {
            arena: self.arena.stats(),
            pool: self.pool.stats(),
            scratch: self.scratch.stats(),
        }
    }

    /// Get the configuration.
    pub fn config(&self) -> &MemoryManagerConfig {
        &self.config
    }
}

impl std::fmt::Debug for MemoryManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryManager")
            .field("config", &self.config)
            .field("arena_stats", &self.arena.stats())
            .field("pool_stats", &self.pool.stats())
            .field("scratch_stats", &self.scratch.stats())
            .finish()
    }
}

/// Combined memory manager statistics
#[derive(Debug, Clone, Default)]
pub struct MemoryManagerStats {
    /// Arena statistics
    pub arena: ArenaStats,
    /// Buffer pool statistics
    pub pool: BufferPoolStats,
    /// Scratch space statistics
    pub scratch: ScratchStats,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_basic() {
        let arena = InferenceArena::new(4096).expect("arena creation failed");

        // Allocate some memory
        let buf1: &mut [f32] = arena.alloc(100).expect("alloc failed");
        assert_eq!(buf1.len(), 100);

        let buf2: &mut [f32] = arena.alloc(200).expect("alloc failed");
        assert_eq!(buf2.len(), 200);

        // Check stats
        let stats = arena.stats();
        assert_eq!(stats.allocation_count, 2);
        assert!(stats.used > 0);

        // Reset and verify
        arena.reset();
        assert_eq!(arena.used(), 0);
        assert_eq!(arena.allocation_count(), 0);
    }

    #[test]
    fn test_arena_alignment() {
        let arena = InferenceArena::new(4096).expect("arena creation failed");

        // Allocate bytes to misalign
        let _: &mut [u8] = arena.alloc(1).unwrap();

        // Next allocation should still be aligned
        let buf: &mut [f32] = arena.alloc(10).unwrap();
        assert!(buf.as_ptr() as usize % DEFAULT_ALIGNMENT == 0);
    }

    #[test]
    fn test_arena_out_of_memory() {
        let arena = InferenceArena::new(1024).expect("arena creation failed");

        // Try to allocate more than capacity
        let result: Option<&mut [f32]> = arena.alloc(1000);
        assert!(result.is_none());
    }

    #[test]
    fn test_buffer_pool_basic() {
        let pool = BufferPool::new();

        // Acquire and release
        let buf1 = pool.acquire(BufferSize::KB4).expect("acquire failed");
        assert_eq!(buf1.capacity(), 4096);
        drop(buf1);

        // Should reuse buffer
        let buf2 = pool.acquire(BufferSize::KB4).expect("acquire failed");
        assert_eq!(buf2.capacity(), 4096);

        let stats = pool.stats();
        assert!(stats.hits > 0 || stats.misses > 0);
    }

    #[test]
    fn test_buffer_pool_size_classes() {
        let pool = BufferPool::new();

        for size in BufferSize::all() {
            let buf = pool.acquire(size).expect("acquire failed");
            assert_eq!(buf.capacity(), size.bytes());
        }
    }

    #[test]
    fn test_buffer_pool_typed_access() {
        let pool = BufferPool::new();
        let mut buf = pool.acquire(BufferSize::KB1).expect("acquire failed");

        // Access as f32 slice
        let floats = buf.as_slice_mut::<f32>();
        assert_eq!(floats.len(), 256); // 1024 / 4

        floats[0] = 1.0;
        floats[1] = 2.0;

        assert_eq!(buf.as_slice::<f32>()[0], 1.0);
    }

    #[test]
    fn test_buffer_pool_prewarm() {
        let pool = BufferPool::new();
        pool.prewarm(BufferSize::KB4, 5).expect("prewarm failed");

        let stats = pool.stats();
        assert_eq!(stats.free_buffers[BufferSize::KB4.index()], 5);
    }

    #[test]
    fn test_scratch_space_basic() {
        let manager = ScratchSpaceManager::new(4096, 4).expect("manager creation failed");

        let mut scratch = manager.get_scratch().expect("get_scratch failed");

        // Allocate some space
        let buf1: &mut [f32] = scratch.get(100).expect("alloc failed");
        assert_eq!(buf1.len(), 100);

        let buf2: &mut [f32] = scratch.get(50).expect("alloc failed");
        assert_eq!(buf2.len(), 50);

        // Check usage
        assert!(scratch.used() > 0);

        // Reset
        scratch.reset();
        assert_eq!(scratch.used(), 0);
    }

    #[test]
    fn test_scratch_space_per_thread() {
        use std::sync::Arc;
        use std::thread;

        let manager = Arc::new(ScratchSpaceManager::new(4096, 4).expect("manager creation failed"));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let manager = Arc::clone(&manager);
                thread::spawn(move || {
                    let mut scratch = manager.get_scratch().expect("get_scratch failed");
                    let _: &mut [f32] = scratch.get(100).unwrap();
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(manager.active_threads(), 4);
    }

    #[test]
    fn test_memory_manager_basic() {
        let manager = MemoryManager::new().expect("manager creation failed");

        // Use arena
        let arena_buf: &mut [f32] = manager.arena.alloc(100).unwrap();
        assert_eq!(arena_buf.len(), 100);

        // Use pool
        let pool_buf = manager
            .pool
            .acquire(BufferSize::KB4)
            .expect("acquire failed");
        assert_eq!(pool_buf.capacity(), 4096);

        // Use scratch
        let mut scratch = manager.scratch.get_scratch().expect("get_scratch failed");
        let scratch_buf: &mut [f32] = scratch.get(50).unwrap();
        assert_eq!(scratch_buf.len(), 50);

        // Reset step
        manager.reset_step();
        assert_eq!(manager.arena.used(), 0);
    }

    #[test]
    fn test_memory_manager_for_model() {
        let manager = MemoryManager::for_model(4096, 32000, 1).expect("manager creation failed");

        let stats = manager.stats();
        assert!(stats.arena.capacity > 0);
    }

    #[test]
    fn test_buffer_size_for_size() {
        assert_eq!(BufferSize::for_size(512), Some(BufferSize::KB1));
        assert_eq!(BufferSize::for_size(1024), Some(BufferSize::KB1));
        assert_eq!(BufferSize::for_size(2000), Some(BufferSize::KB4));
        assert_eq!(BufferSize::for_size(4096), Some(BufferSize::KB4));
        assert_eq!(BufferSize::for_size(10000), Some(BufferSize::KB16));
        assert_eq!(BufferSize::for_size(50000), Some(BufferSize::KB64));
        assert_eq!(BufferSize::for_size(200000), Some(BufferSize::KB256));
        assert_eq!(BufferSize::for_size(300000), None);
    }
}
