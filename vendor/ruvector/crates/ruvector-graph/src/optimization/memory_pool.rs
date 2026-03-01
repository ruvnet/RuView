//! Custom memory allocators for graph query execution
//!
//! This module provides specialized allocators:
//! - Arena allocation for query-scoped memory
//! - Object pooling for frequent allocations
//! - NUMA-aware allocation for distributed systems

use parking_lot::Mutex;
use std::alloc::{alloc, dealloc, Layout};
use std::cell::Cell;
use std::ptr::{self, NonNull};
use std::sync::Arc;

/// Arena allocator for query execution
/// All allocations are freed together when the arena is dropped
pub struct ArenaAllocator {
    /// Current chunk
    current: Cell<Option<NonNull<Chunk>>>,
    /// All chunks (for cleanup)
    chunks: Mutex<Vec<NonNull<Chunk>>>,
    /// Default chunk size
    chunk_size: usize,
}

struct Chunk {
    /// Data buffer
    data: NonNull<u8>,
    /// Current offset in buffer
    offset: Cell<usize>,
    /// Total capacity
    capacity: usize,
    /// Next chunk in linked list
    next: Cell<Option<NonNull<Chunk>>>,
}

impl ArenaAllocator {
    /// Create a new arena with default chunk size (1MB)
    pub fn new() -> Self {
        Self::with_chunk_size(1024 * 1024)
    }

    /// Create arena with specific chunk size
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Self {
            current: Cell::new(None),
            chunks: Mutex::new(Vec::new()),
            chunk_size,
        }
    }

    /// Allocate memory from the arena
    pub fn alloc<T>(&self) -> NonNull<T> {
        let layout = Layout::new::<T>();
        let ptr = self.alloc_layout(layout);
        ptr.cast()
    }

    /// Allocate with specific layout
    pub fn alloc_layout(&self, layout: Layout) -> NonNull<u8> {
        let size = layout.size();
        let align = layout.align();

        // SECURITY: Validate layout parameters
        assert!(size > 0, "Cannot allocate zero bytes");
        assert!(
            align > 0 && align.is_power_of_two(),
            "Alignment must be a power of 2"
        );
        assert!(size <= isize::MAX as usize, "Allocation size too large");

        // Get current chunk or allocate new one
        let chunk = match self.current.get() {
            Some(chunk) => chunk,
            None => {
                let chunk = self.allocate_chunk();
                self.current.set(Some(chunk));
                chunk
            }
        };

        unsafe {
            let chunk_ref = chunk.as_ref();
            let offset = chunk_ref.offset.get();

            // Align offset
            let aligned_offset = (offset + align - 1) & !(align - 1);

            // SECURITY: Check for overflow in alignment calculation
            if aligned_offset < offset {
                panic!("Alignment calculation overflow");
            }

            let new_offset = aligned_offset
                .checked_add(size)
                .expect("Arena allocation overflow");

            if new_offset > chunk_ref.capacity {
                // Need a new chunk
                let new_chunk = self.allocate_chunk();
                chunk_ref.next.set(Some(new_chunk));
                self.current.set(Some(new_chunk));

                // Retry allocation with new chunk
                return self.alloc_layout(layout);
            }

            chunk_ref.offset.set(new_offset);

            // SECURITY: Verify pointer arithmetic is safe
            let result_ptr = chunk_ref.data.as_ptr().add(aligned_offset);
            debug_assert!(
                result_ptr as usize >= chunk_ref.data.as_ptr() as usize,
                "Pointer arithmetic underflow"
            );
            debug_assert!(
                result_ptr as usize <= chunk_ref.data.as_ptr().add(chunk_ref.capacity) as usize,
                "Pointer arithmetic overflow"
            );

            NonNull::new_unchecked(result_ptr)
        }
    }

    /// Allocate a new chunk
    fn allocate_chunk(&self) -> NonNull<Chunk> {
        unsafe {
            let layout = Layout::from_size_align_unchecked(self.chunk_size, 64);
            let data = NonNull::new_unchecked(alloc(layout));

            let chunk_layout = Layout::new::<Chunk>();
            let chunk_ptr = alloc(chunk_layout) as *mut Chunk;

            ptr::write(
                chunk_ptr,
                Chunk {
                    data,
                    offset: Cell::new(0),
                    capacity: self.chunk_size,
                    next: Cell::new(None),
                },
            );

            let chunk = NonNull::new_unchecked(chunk_ptr);
            self.chunks.lock().push(chunk);
            chunk
        }
    }

    /// Reset arena (reuse existing chunks)
    pub fn reset(&self) {
        let chunks = self.chunks.lock();
        for &chunk in chunks.iter() {
            unsafe {
                chunk.as_ref().offset.set(0);
                chunk.as_ref().next.set(None);
            }
        }

        if let Some(first_chunk) = chunks.first() {
            self.current.set(Some(*first_chunk));
        }
    }

    /// Get total allocated bytes across all chunks
    pub fn total_allocated(&self) -> usize {
        self.chunks.lock().len() * self.chunk_size
    }
}

impl Default for ArenaAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ArenaAllocator {
    fn drop(&mut self) {
        let chunks = self.chunks.lock();
        for &chunk in chunks.iter() {
            unsafe {
                let chunk_ref = chunk.as_ref();

                // Deallocate data buffer
                let data_layout = Layout::from_size_align_unchecked(chunk_ref.capacity, 64);
                dealloc(chunk_ref.data.as_ptr(), data_layout);

                // Deallocate chunk itself
                let chunk_layout = Layout::new::<Chunk>();
                dealloc(chunk.as_ptr() as *mut u8, chunk_layout);
            }
        }
    }
}

unsafe impl Send for ArenaAllocator {}
unsafe impl Sync for ArenaAllocator {}

/// Query-scoped arena that resets after each query
pub struct QueryArena {
    arena: Arc<ArenaAllocator>,
}

impl QueryArena {
    pub fn new() -> Self {
        Self {
            arena: Arc::new(ArenaAllocator::new()),
        }
    }

    pub fn execute_query<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&ArenaAllocator) -> R,
    {
        let result = f(&self.arena);
        self.arena.reset();
        result
    }

    pub fn arena(&self) -> &ArenaAllocator {
        &self.arena
    }
}

impl Default for QueryArena {
    fn default() -> Self {
        Self::new()
    }
}

/// NUMA-aware allocator for multi-socket systems
pub struct NumaAllocator {
    /// Allocators per NUMA node
    node_allocators: Vec<Arc<ArenaAllocator>>,
    /// Current thread's preferred NUMA node
    preferred_node: Cell<usize>,
}

impl NumaAllocator {
    /// Create NUMA-aware allocator
    pub fn new() -> Self {
        let num_nodes = Self::detect_numa_nodes();
        let node_allocators = (0..num_nodes)
            .map(|_| Arc::new(ArenaAllocator::new()))
            .collect();

        Self {
            node_allocators,
            preferred_node: Cell::new(0),
        }
    }

    /// Detect number of NUMA nodes (simplified)
    fn detect_numa_nodes() -> usize {
        // In a real implementation, this would use platform-specific APIs
        // For now, assume 1 node per 8 CPUs
        let cpus = num_cpus::get();
        ((cpus + 7) / 8).max(1)
    }

    /// Allocate from preferred NUMA node
    pub fn alloc<T>(&self) -> NonNull<T> {
        let node = self.preferred_node.get();
        self.node_allocators[node].alloc()
    }

    /// Set preferred NUMA node for current thread
    pub fn set_preferred_node(&self, node: usize) {
        if node < self.node_allocators.len() {
            self.preferred_node.set(node);
        }
    }

    /// Bind current thread to NUMA node
    pub fn bind_to_node(&self, node: usize) {
        self.set_preferred_node(node);

        // In a real implementation, this would use platform-specific APIs
        // to bind the thread to CPUs on the specified NUMA node
        #[cfg(target_os = "linux")]
        {
            // Would use libnuma or similar
        }
    }
}

impl Default for NumaAllocator {
    fn default() -> Self {
        Self::new()
    }
}

/// Object pool for reducing allocation overhead
pub struct ObjectPool<T> {
    /// Pool of available objects
    available: Arc<crossbeam::queue::SegQueue<T>>,
    /// Factory function
    factory: Arc<dyn Fn() -> T + Send + Sync>,
    /// Maximum pool size
    max_size: usize,
}

impl<T> ObjectPool<T> {
    pub fn new<F>(max_size: usize, factory: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            available: Arc::new(crossbeam::queue::SegQueue::new()),
            factory: Arc::new(factory),
            max_size,
        }
    }

    pub fn acquire(&self) -> PooledObject<T> {
        let object = self.available.pop().unwrap_or_else(|| (self.factory)());

        PooledObject {
            object: Some(object),
            pool: Arc::clone(&self.available),
        }
    }

    pub fn len(&self) -> usize {
        self.available.len()
    }

    pub fn is_empty(&self) -> bool {
        self.available.is_empty()
    }
}

/// RAII wrapper for pooled objects
pub struct PooledObject<T> {
    object: Option<T>,
    pool: Arc<crossbeam::queue::SegQueue<T>>,
}

impl<T> PooledObject<T> {
    pub fn get(&self) -> &T {
        self.object.as_ref().unwrap()
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.object.as_mut().unwrap()
    }
}

impl<T> Drop for PooledObject<T> {
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            let _ = self.pool.push(object);
        }
    }
}

impl<T> std::ops::Deref for PooledObject<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.object.as_ref().unwrap()
    }
}

impl<T> std::ops::DerefMut for PooledObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.object.as_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_allocator() {
        let arena = ArenaAllocator::new();

        let ptr1 = arena.alloc::<u64>();
        let ptr2 = arena.alloc::<u64>();

        unsafe {
            ptr1.as_ptr().write(42);
            ptr2.as_ptr().write(84);

            assert_eq!(ptr1.as_ptr().read(), 42);
            assert_eq!(ptr2.as_ptr().read(), 84);
        }
    }

    #[test]
    fn test_arena_reset() {
        let arena = ArenaAllocator::new();

        for _ in 0..100 {
            arena.alloc::<u64>();
        }

        let allocated_before = arena.total_allocated();
        arena.reset();
        let allocated_after = arena.total_allocated();

        assert_eq!(allocated_before, allocated_after);
    }

    #[test]
    fn test_query_arena() {
        let query_arena = QueryArena::new();

        let result = query_arena.execute_query(|arena| {
            let ptr = arena.alloc::<u64>();
            unsafe {
                ptr.as_ptr().write(123);
                ptr.as_ptr().read()
            }
        });

        assert_eq!(result, 123);
    }

    #[test]
    fn test_object_pool() {
        let pool = ObjectPool::new(10, || Vec::<u8>::with_capacity(1024));

        let mut obj = pool.acquire();
        obj.push(42);
        assert_eq!(obj[0], 42);

        drop(obj);

        let obj2 = pool.acquire();
        assert!(obj2.capacity() >= 1024);
    }
}
