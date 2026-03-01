//! Arena allocator for batch operations
//!
//! This module provides arena-based memory allocation to reduce allocation
//! overhead in hot paths and improve memory locality.
//!
//! ## Features (ADR-001)
//!
//! - **Cache-aligned allocations**: All allocations are aligned to cache line boundaries (64 bytes)
//! - **Bump allocation**: O(1) allocation with minimal overhead
//! - **Batch deallocation**: Free all allocations at once via `reset()`
//! - **Thread-local arenas**: Per-thread allocation without synchronization

use std::alloc::{alloc, dealloc, Layout};
use std::cell::RefCell;
use std::ptr;

/// Cache line size (typically 64 bytes on modern CPUs)
pub const CACHE_LINE_SIZE: usize = 64;

/// Arena allocator for temporary allocations
///
/// Use this for batch operations where many temporary allocations
/// are needed and can be freed all at once.
pub struct Arena {
    chunks: RefCell<Vec<Chunk>>,
    chunk_size: usize,
}

struct Chunk {
    data: *mut u8,
    capacity: usize,
    used: usize,
}

impl Arena {
    /// Create a new arena with the specified chunk size
    pub fn new(chunk_size: usize) -> Self {
        Self {
            chunks: RefCell::new(Vec::new()),
            chunk_size,
        }
    }

    /// Create an arena with a default 1MB chunk size
    pub fn with_default_chunk_size() -> Self {
        Self::new(1024 * 1024) // 1MB
    }

    /// Allocate a buffer of the specified size
    pub fn alloc_vec<T>(&self, count: usize) -> ArenaVec<T> {
        let size = count * std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();

        let ptr = self.alloc_raw(size, align);

        ArenaVec {
            ptr: ptr as *mut T,
            len: 0,
            capacity: count,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Allocate raw bytes with specified alignment
    fn alloc_raw(&self, size: usize, align: usize) -> *mut u8 {
        // SECURITY: Validate alignment is a power of 2 and size is reasonable
        assert!(
            align > 0 && align.is_power_of_two(),
            "Alignment must be a power of 2"
        );
        assert!(size > 0, "Cannot allocate zero bytes");
        assert!(size <= isize::MAX as usize, "Allocation size too large");

        let mut chunks = self.chunks.borrow_mut();

        // Try to allocate from the last chunk
        if let Some(chunk) = chunks.last_mut() {
            // Align the current position
            let current = chunk.used;
            let aligned = (current + align - 1) & !(align - 1);

            // SECURITY: Check for overflow in alignment calculation
            if aligned < current {
                panic!("Alignment calculation overflow");
            }

            let needed = aligned
                .checked_add(size)
                .expect("Arena allocation size overflow");

            if needed <= chunk.capacity {
                chunk.used = needed;
                return unsafe {
                    // SECURITY: Verify pointer arithmetic doesn't overflow
                    let ptr = chunk.data.add(aligned);
                    debug_assert!(ptr as usize >= chunk.data as usize, "Pointer underflow");
                    ptr
                };
            }
        }

        // Need a new chunk
        let chunk_size = self.chunk_size.max(size + align);
        let layout = Layout::from_size_align(chunk_size, 64).unwrap();
        let data = unsafe { alloc(layout) };

        let aligned = align;
        let chunk = Chunk {
            data,
            capacity: chunk_size,
            used: aligned + size,
        };

        let ptr = unsafe { data.add(aligned) };
        chunks.push(chunk);

        ptr
    }

    /// Reset the arena, allowing reuse of allocated memory
    pub fn reset(&self) {
        let mut chunks = self.chunks.borrow_mut();
        for chunk in chunks.iter_mut() {
            chunk.used = 0;
        }
    }

    /// Get total allocated bytes
    pub fn allocated_bytes(&self) -> usize {
        let chunks = self.chunks.borrow();
        chunks.iter().map(|c| c.capacity).sum()
    }

    /// Get used bytes
    pub fn used_bytes(&self) -> usize {
        let chunks = self.chunks.borrow();
        chunks.iter().map(|c| c.used).sum()
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        let chunks = self.chunks.borrow();
        for chunk in chunks.iter() {
            let layout = Layout::from_size_align(chunk.capacity, 64).unwrap();
            unsafe {
                dealloc(chunk.data, layout);
            }
        }
    }
}

/// Vector allocated from an arena
pub struct ArenaVec<T> {
    ptr: *mut T,
    len: usize,
    capacity: usize,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> ArenaVec<T> {
    /// Push an element (panics if capacity exceeded)
    pub fn push(&mut self, value: T) {
        // SECURITY: Bounds check before pointer arithmetic
        assert!(self.len < self.capacity, "ArenaVec capacity exceeded");
        assert!(!self.ptr.is_null(), "ArenaVec pointer is null");

        unsafe {
            // Additional safety: verify the pointer offset is within bounds
            let offset_ptr = self.ptr.add(self.len);
            debug_assert!(
                offset_ptr as usize >= self.ptr as usize,
                "Pointer arithmetic overflow"
            );
            ptr::write(offset_ptr, value);
        }
        self.len += 1;
    }

    /// Get length
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get as slice
    pub fn as_slice(&self) -> &[T] {
        // SECURITY: Bounds check before creating slice
        assert!(self.len <= self.capacity, "Length exceeds capacity");
        assert!(!self.ptr.is_null(), "Cannot create slice from null pointer");

        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// Get as mutable slice
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SECURITY: Bounds check before creating slice
        assert!(self.len <= self.capacity, "Length exceeds capacity");
        assert!(!self.ptr.is_null(), "Cannot create slice from null pointer");

        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl<T> std::ops::Deref for ArenaVec<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> std::ops::DerefMut for ArenaVec<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

// Thread-local arena for per-thread allocations
thread_local! {
    static THREAD_ARENA: RefCell<Arena> = RefCell::new(Arena::with_default_chunk_size());
}

// Get the thread-local arena
// Note: Commented out due to lifetime issues with RefCell::borrow() escaping closure
// Use THREAD_ARENA.with(|arena| { ... }) directly instead
/*
pub fn thread_arena() -> impl std::ops::Deref<Target = Arena> {
    THREAD_ARENA.with(|arena| {
        arena.borrow()
    })
}
*/

/// Cache-aligned vector storage for SIMD operations (ADR-001)
///
/// Ensures vectors are aligned to cache line boundaries (64 bytes) for
/// optimal SIMD operations and minimal cache misses.
#[repr(C, align(64))]
pub struct CacheAlignedVec {
    data: *mut f32,
    len: usize,
    capacity: usize,
}

impl CacheAlignedVec {
    /// Create a new cache-aligned vector with the given capacity
    ///
    /// # Panics
    ///
    /// Panics if memory allocation fails. For fallible allocation,
    /// use `try_with_capacity`.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::try_with_capacity(capacity).expect("Failed to allocate cache-aligned memory")
    }

    /// Try to create a new cache-aligned vector with the given capacity
    ///
    /// Returns `None` if memory allocation fails.
    pub fn try_with_capacity(capacity: usize) -> Option<Self> {
        // Handle zero capacity case
        if capacity == 0 {
            return Some(Self {
                data: std::ptr::null_mut(),
                len: 0,
                capacity: 0,
            });
        }

        // Allocate cache-line aligned memory
        let layout =
            Layout::from_size_align(capacity * std::mem::size_of::<f32>(), CACHE_LINE_SIZE).ok()?;

        let data = unsafe { alloc(layout) as *mut f32 };

        // SECURITY: Check for allocation failure
        if data.is_null() {
            return None;
        }

        Some(Self {
            data,
            len: 0,
            capacity,
        })
    }

    /// Create from an existing slice, copying data to cache-aligned storage
    ///
    /// # Panics
    ///
    /// Panics if memory allocation fails. For fallible allocation,
    /// use `try_from_slice`.
    pub fn from_slice(slice: &[f32]) -> Self {
        Self::try_from_slice(slice).expect("Failed to allocate cache-aligned memory for slice")
    }

    /// Try to create from an existing slice, copying data to cache-aligned storage
    ///
    /// Returns `None` if memory allocation fails.
    pub fn try_from_slice(slice: &[f32]) -> Option<Self> {
        let mut vec = Self::try_with_capacity(slice.len())?;
        if !slice.is_empty() {
            unsafe {
                ptr::copy_nonoverlapping(slice.as_ptr(), vec.data, slice.len());
            }
        }
        vec.len = slice.len();
        Some(vec)
    }

    /// Push an element
    ///
    /// # Panics
    ///
    /// Panics if capacity is exceeded or if the vector has zero capacity.
    pub fn push(&mut self, value: f32) {
        assert!(
            self.len < self.capacity,
            "CacheAlignedVec capacity exceeded"
        );
        assert!(
            !self.data.is_null(),
            "Cannot push to zero-capacity CacheAlignedVec"
        );
        unsafe {
            *self.data.add(self.len) = value;
        }
        self.len += 1;
    }

    /// Get length
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get capacity
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get as slice
    #[inline]
    pub fn as_slice(&self) -> &[f32] {
        if self.len == 0 {
            // SAFETY: Empty slice doesn't require valid pointer
            return &[];
        }
        // SAFETY: data is valid for len elements when len > 0
        unsafe { std::slice::from_raw_parts(self.data, self.len) }
    }

    /// Get as mutable slice
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [f32] {
        if self.len == 0 {
            // SAFETY: Empty slice doesn't require valid pointer
            return &mut [];
        }
        // SAFETY: data is valid for len elements when len > 0
        unsafe { std::slice::from_raw_parts_mut(self.data, self.len) }
    }

    /// Get raw pointer (for SIMD operations)
    #[inline]
    pub fn as_ptr(&self) -> *const f32 {
        self.data
    }

    /// Get mutable raw pointer (for SIMD operations)
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut f32 {
        self.data
    }

    /// Check if properly aligned for SIMD
    ///
    /// Returns `true` for zero-capacity vectors (considered trivially aligned).
    #[inline]
    pub fn is_aligned(&self) -> bool {
        if self.data.is_null() {
            // Zero-capacity vectors are considered aligned
            return self.capacity == 0;
        }
        (self.data as usize) % CACHE_LINE_SIZE == 0
    }

    /// Clear the vector (sets len to 0, doesn't deallocate)
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

impl Drop for CacheAlignedVec {
    fn drop(&mut self) {
        if !self.data.is_null() && self.capacity > 0 {
            let layout = Layout::from_size_align(
                self.capacity * std::mem::size_of::<f32>(),
                CACHE_LINE_SIZE,
            )
            .expect("Invalid layout");

            unsafe {
                dealloc(self.data as *mut u8, layout);
            }
        }
    }
}

impl std::ops::Deref for CacheAlignedVec {
    type Target = [f32];

    fn deref(&self) -> &[f32] {
        self.as_slice()
    }
}

impl std::ops::DerefMut for CacheAlignedVec {
    fn deref_mut(&mut self) -> &mut [f32] {
        self.as_mut_slice()
    }
}

// Safety: The raw pointer is owned and not shared
unsafe impl Send for CacheAlignedVec {}
unsafe impl Sync for CacheAlignedVec {}

/// Batch vector allocator for processing multiple vectors (ADR-001)
///
/// Allocates contiguous, cache-aligned storage for a batch of vectors,
/// enabling efficient SIMD processing and minimal cache misses.
pub struct BatchVectorAllocator {
    data: *mut f32,
    dimensions: usize,
    capacity: usize,
    count: usize,
}

impl BatchVectorAllocator {
    /// Create allocator for vectors of given dimensions
    ///
    /// # Panics
    ///
    /// Panics if memory allocation fails. For fallible allocation,
    /// use `try_new`.
    pub fn new(dimensions: usize, initial_capacity: usize) -> Self {
        Self::try_new(dimensions, initial_capacity)
            .expect("Failed to allocate batch vector storage")
    }

    /// Try to create allocator for vectors of given dimensions
    ///
    /// Returns `None` if memory allocation fails.
    pub fn try_new(dimensions: usize, initial_capacity: usize) -> Option<Self> {
        // Handle zero capacity case
        if dimensions == 0 || initial_capacity == 0 {
            return Some(Self {
                data: std::ptr::null_mut(),
                dimensions,
                capacity: initial_capacity,
                count: 0,
            });
        }

        let total_floats = dimensions * initial_capacity;

        let layout =
            Layout::from_size_align(total_floats * std::mem::size_of::<f32>(), CACHE_LINE_SIZE)
                .ok()?;

        let data = unsafe { alloc(layout) as *mut f32 };

        // SECURITY: Check for allocation failure
        if data.is_null() {
            return None;
        }

        Some(Self {
            data,
            dimensions,
            capacity: initial_capacity,
            count: 0,
        })
    }

    /// Add a vector, returns its index
    ///
    /// # Panics
    ///
    /// Panics if the allocator is full, dimensions mismatch, or allocator has zero capacity.
    pub fn add(&mut self, vector: &[f32]) -> usize {
        assert_eq!(vector.len(), self.dimensions, "Vector dimension mismatch");
        assert!(self.count < self.capacity, "Batch allocator full");
        assert!(
            !self.data.is_null(),
            "Cannot add to zero-capacity BatchVectorAllocator"
        );

        let offset = self.count * self.dimensions;
        unsafe {
            ptr::copy_nonoverlapping(vector.as_ptr(), self.data.add(offset), self.dimensions);
        }

        let index = self.count;
        self.count += 1;
        index
    }

    /// Get a vector by index
    pub fn get(&self, index: usize) -> &[f32] {
        assert!(index < self.count, "Index out of bounds");
        let offset = index * self.dimensions;
        unsafe { std::slice::from_raw_parts(self.data.add(offset), self.dimensions) }
    }

    /// Get mutable vector by index
    pub fn get_mut(&mut self, index: usize) -> &mut [f32] {
        assert!(index < self.count, "Index out of bounds");
        let offset = index * self.dimensions;
        unsafe { std::slice::from_raw_parts_mut(self.data.add(offset), self.dimensions) }
    }

    /// Get raw pointer to vector at index (for SIMD)
    #[inline]
    pub fn ptr_at(&self, index: usize) -> *const f32 {
        assert!(index < self.count, "Index out of bounds");
        let offset = index * self.dimensions;
        unsafe { self.data.add(offset) }
    }

    /// Number of vectors stored
    #[inline]
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Dimensions per vector
    #[inline]
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Reset allocator (keeps memory)
    pub fn clear(&mut self) {
        self.count = 0;
    }
}

impl Drop for BatchVectorAllocator {
    fn drop(&mut self) {
        if !self.data.is_null() {
            let layout = Layout::from_size_align(
                self.dimensions * self.capacity * std::mem::size_of::<f32>(),
                CACHE_LINE_SIZE,
            )
            .expect("Invalid layout");

            unsafe {
                dealloc(self.data as *mut u8, layout);
            }
        }
    }
}

// Safety: The raw pointer is owned and not shared
unsafe impl Send for BatchVectorAllocator {}
unsafe impl Sync for BatchVectorAllocator {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_alloc() {
        let arena = Arena::new(1024);

        let mut vec1 = arena.alloc_vec::<f32>(10);
        vec1.push(1.0);
        vec1.push(2.0);
        vec1.push(3.0);

        assert_eq!(vec1.len(), 3);
        assert_eq!(vec1[0], 1.0);
        assert_eq!(vec1[1], 2.0);
        assert_eq!(vec1[2], 3.0);
    }

    #[test]
    fn test_arena_multiple_allocs() {
        let arena = Arena::new(1024);

        let vec1 = arena.alloc_vec::<u32>(100);
        let vec2 = arena.alloc_vec::<u64>(50);
        let vec3 = arena.alloc_vec::<f32>(200);

        assert_eq!(vec1.capacity(), 100);
        assert_eq!(vec2.capacity(), 50);
        assert_eq!(vec3.capacity(), 200);
    }

    #[test]
    fn test_arena_reset() {
        let arena = Arena::new(1024);

        {
            let _vec1 = arena.alloc_vec::<f32>(100);
            let _vec2 = arena.alloc_vec::<f32>(100);
        }

        let used_before = arena.used_bytes();
        arena.reset();
        let used_after = arena.used_bytes();

        assert!(used_after < used_before);
    }

    #[test]
    fn test_cache_aligned_vec() {
        let mut vec = CacheAlignedVec::with_capacity(100);

        // Check alignment
        assert!(vec.is_aligned(), "Vector should be cache-aligned");

        // Test push
        for i in 0..50 {
            vec.push(i as f32);
        }
        assert_eq!(vec.len(), 50);

        // Test slice access
        let slice = vec.as_slice();
        assert_eq!(slice[0], 0.0);
        assert_eq!(slice[49], 49.0);
    }

    #[test]
    fn test_cache_aligned_vec_from_slice() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let aligned = CacheAlignedVec::from_slice(&data);

        assert!(aligned.is_aligned());
        assert_eq!(aligned.len(), 5);
        assert_eq!(aligned.as_slice(), &data[..]);
    }

    #[test]
    fn test_batch_vector_allocator() {
        let mut allocator = BatchVectorAllocator::new(4, 10);

        let v1 = vec![1.0, 2.0, 3.0, 4.0];
        let v2 = vec![5.0, 6.0, 7.0, 8.0];

        let idx1 = allocator.add(&v1);
        let idx2 = allocator.add(&v2);

        assert_eq!(idx1, 0);
        assert_eq!(idx2, 1);
        assert_eq!(allocator.len(), 2);

        // Test retrieval
        assert_eq!(allocator.get(0), &v1[..]);
        assert_eq!(allocator.get(1), &v2[..]);
    }

    #[test]
    fn test_batch_allocator_clear() {
        let mut allocator = BatchVectorAllocator::new(3, 5);

        allocator.add(&[1.0, 2.0, 3.0]);
        allocator.add(&[4.0, 5.0, 6.0]);

        assert_eq!(allocator.len(), 2);

        allocator.clear();
        assert_eq!(allocator.len(), 0);

        // Should be able to add again
        allocator.add(&[7.0, 8.0, 9.0]);
        assert_eq!(allocator.len(), 1);
    }
}
