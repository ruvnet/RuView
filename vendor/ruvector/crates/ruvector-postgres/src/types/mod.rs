//! Vector type implementations for PostgreSQL with zero-copy optimizations
//!
//! This module provides the core vector types with optimized memory layouts:
//! - `RuVector`: Primary f32 vector type (pgvector compatible)
//! - `HalfVec`: Half-precision (f16) vector for memory savings
//! - `SparseVec`: Sparse vector for high-dimensional data
//!
//! Features:
//! - Zero-copy data access via VectorData trait
//! - PostgreSQL memory context integration
//! - Shared memory structures for indexes
//! - TOAST handling for large vectors
//! - Optimized memory layouts

mod binaryvec;
mod halfvec;
mod productvec;
mod scalarvec;
mod sparsevec;
pub mod vector;

pub use binaryvec::BinaryVec;
pub use halfvec::HalfVec;
pub use productvec::ProductVec;
pub use scalarvec::ScalarVec;
pub use sparsevec::SparseVec;
pub use vector::RuVector;

use pgrx::prelude::*;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

/// Global vector cache memory tracking
static VECTOR_CACHE_BYTES: AtomicUsize = AtomicUsize::new(0);

/// Get current vector cache memory usage in MB
pub fn get_vector_cache_memory_mb() -> f64 {
    VECTOR_CACHE_BYTES.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
}

/// Track memory allocation
pub(crate) fn track_allocation(bytes: usize) {
    VECTOR_CACHE_BYTES.fetch_add(bytes, Ordering::Relaxed);
}

/// Track memory deallocation
pub(crate) fn track_deallocation(bytes: usize) {
    VECTOR_CACHE_BYTES.fetch_sub(bytes, Ordering::Relaxed);
}

// ============================================================================
// Zero-Copy Vector Data Interface
// ============================================================================

/// Common trait for all vector types with zero-copy access
///
/// This trait provides a unified interface for accessing vector data
/// without copying, enabling efficient SIMD operations and memory sharing.
///
/// # Safety
///
/// Implementations must ensure that `data_ptr()` returns a valid pointer
/// to properly aligned f32 data that remains valid for the lifetime of the object.
pub trait VectorData {
    /// Get raw pointer to f32 data (zero-copy access)
    ///
    /// # Safety
    ///
    /// The returned pointer must point to valid, aligned f32 data
    /// for at least `dimensions()` elements.
    unsafe fn data_ptr(&self) -> *const f32;

    /// Get mutable pointer to f32 data (zero-copy access)
    ///
    /// # Safety
    ///
    /// The returned pointer must point to valid, aligned f32 data
    /// for at least `dimensions()` elements.
    unsafe fn data_ptr_mut(&mut self) -> *mut f32;

    /// Get vector dimensions
    fn dimensions(&self) -> usize;

    /// Get data as slice (zero-copy if possible)
    ///
    /// For types that store f32 directly, this is zero-copy.
    /// For types like HalfVec, this may require conversion.
    fn as_slice(&self) -> &[f32];

    /// Get mutable data slice
    fn as_mut_slice(&mut self) -> &mut [f32];

    /// Total memory size in bytes (including metadata)
    fn memory_size(&self) -> usize;

    /// Memory size of the data portion only
    fn data_size(&self) -> usize {
        self.dimensions() * std::mem::size_of::<f32>()
    }

    /// Check if data is aligned for SIMD operations
    fn is_simd_aligned(&self) -> bool {
        const ALIGNMENT: usize = 64; // AVX-512 alignment
        unsafe { (self.data_ptr() as usize) % ALIGNMENT == 0 }
    }

    /// Check if vector is stored inline (not TOASTed)
    fn is_inline(&self) -> bool {
        self.memory_size() < TOAST_THRESHOLD
    }
}

/// TOAST threshold: vectors larger than this may be compressed/externalized
/// PostgreSQL TOAST threshold is typically 2KB
pub const TOAST_THRESHOLD: usize = 2000;

/// Inline storage limit for small vectors
pub const INLINE_THRESHOLD: usize = 512;

// ============================================================================
// PostgreSQL Memory Context Integration
// ============================================================================

/// PostgreSQL memory context for vector allocation
#[repr(C)]
pub struct PgVectorContext {
    /// Total allocated bytes
    pub total_bytes: AtomicUsize,
    /// Number of vectors allocated
    pub vector_count: AtomicU32,
    /// Peak memory usage
    pub peak_bytes: AtomicUsize,
}

impl PgVectorContext {
    /// Create a new memory context
    pub fn new() -> Self {
        Self {
            total_bytes: AtomicUsize::new(0),
            vector_count: AtomicU32::new(0),
            peak_bytes: AtomicUsize::new(0),
        }
    }

    /// Track allocation
    pub fn track_alloc(&self, bytes: usize) {
        let new_total = self.total_bytes.fetch_add(bytes, Ordering::Relaxed) + bytes;
        self.vector_count.fetch_add(1, Ordering::Relaxed);

        // Update peak if necessary
        let mut peak = self.peak_bytes.load(Ordering::Relaxed);
        while new_total > peak {
            match self.peak_bytes.compare_exchange_weak(
                peak,
                new_total,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => peak = x,
            }
        }
    }

    /// Track deallocation
    pub fn track_dealloc(&self, bytes: usize) {
        self.total_bytes.fetch_sub(bytes, Ordering::Relaxed);
        self.vector_count.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get current memory usage in bytes
    pub fn current_bytes(&self) -> usize {
        self.total_bytes.load(Ordering::Relaxed)
    }

    /// Get peak memory usage in bytes
    pub fn peak_bytes(&self) -> usize {
        self.peak_bytes.load(Ordering::Relaxed)
    }

    /// Get vector count
    pub fn count(&self) -> u32 {
        self.vector_count.load(Ordering::Relaxed)
    }
}

impl Default for PgVectorContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Global memory context for vectors
static GLOBAL_VECTOR_CONTEXT: PgVectorContext = PgVectorContext {
    total_bytes: AtomicUsize::new(0),
    vector_count: AtomicU32::new(0),
    peak_bytes: AtomicUsize::new(0),
};

/// Allocate vector in PostgreSQL memory context
///
/// This allocates memory using PostgreSQL's palloc, which automatically
/// handles memory cleanup when the transaction ends.
///
/// # Safety
///
/// The returned pointer is owned by PostgreSQL and will be freed
/// when the memory context is reset.
pub unsafe fn palloc_vector(dims: usize) -> *mut u8 {
    let data_size = dims * std::mem::size_of::<f32>();
    let header_size = std::mem::size_of::<VectorHeader>();
    let total_size = header_size + data_size;

    let ptr = pg_sys::palloc(total_size) as *mut u8;

    // Track allocation
    GLOBAL_VECTOR_CONTEXT.track_alloc(total_size);

    ptr
}

/// Allocate aligned vector in PostgreSQL memory context
///
/// Allocates memory aligned for SIMD operations (64-byte alignment for AVX-512)
///
/// # Safety
///
/// The returned pointer is owned by PostgreSQL and will be freed
/// when the memory context is reset.
pub unsafe fn palloc_vector_aligned(dims: usize) -> *mut u8 {
    let data_size = dims * std::mem::size_of::<f32>();
    let header_size = std::mem::size_of::<VectorHeader>();
    let total_size = header_size + data_size;

    // Add padding for alignment
    const ALIGNMENT: usize = 64;
    let aligned_size = (total_size + ALIGNMENT - 1) & !(ALIGNMENT - 1);

    let ptr = pg_sys::palloc(aligned_size) as *mut u8;

    // Align pointer
    let aligned = (ptr as usize + ALIGNMENT - 1) & !(ALIGNMENT - 1);

    // Track allocation
    GLOBAL_VECTOR_CONTEXT.track_alloc(aligned_size);

    aligned as *mut u8
}

/// Free vector memory (if allocated with custom allocator)
///
/// # Safety
///
/// The pointer must have been allocated with palloc_vector or palloc_vector_aligned
pub unsafe fn pfree_vector(ptr: *mut u8, dims: usize) {
    let data_size = dims * std::mem::size_of::<f32>();
    let header_size = std::mem::size_of::<VectorHeader>();
    let total_size = header_size + data_size;

    pg_sys::pfree(ptr as *mut std::os::raw::c_void);

    // Track deallocation
    GLOBAL_VECTOR_CONTEXT.track_dealloc(total_size);
}

/// Vector header for PostgreSQL storage
///
/// This matches the PostgreSQL varlena header format:
/// - First 4 bytes: varlena header (total size including header)
/// - Next 4 bytes: dimensions
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct VectorHeader {
    /// Total size in bytes (varlena format)
    pub vl_len: u32,
    /// Number of dimensions
    pub dimensions: u32,
}

impl VectorHeader {
    /// Create a new vector header
    pub fn new(dimensions: u32, data_size: usize) -> Self {
        let total_size = std::mem::size_of::<Self>() + data_size;
        Self {
            vl_len: total_size as u32,
            dimensions,
        }
    }

    /// Get total size
    pub fn total_size(&self) -> usize {
        self.vl_len as usize
    }

    /// Get data size
    pub fn data_size(&self) -> usize {
        self.total_size() - std::mem::size_of::<Self>()
    }

    /// Check if vector is TOASTed (external storage)
    pub fn is_toasted(&self) -> bool {
        // In PostgreSQL, if the first byte has the high bit set differently,
        // it indicates TOAST compression/external storage
        (self.vl_len & 0x8000_0000) != 0
    }
}

// ============================================================================
// Shared Memory Structures for Indexes
// ============================================================================

/// Shared memory segment for HNSW index
///
/// This structure is stored in PostgreSQL shared memory and can be
/// accessed by multiple backends concurrently.
#[repr(C, align(64))] // Cache-line aligned
pub struct HnswSharedMem {
    /// Entry point node ID (atomic for concurrent access)
    pub entry_point: AtomicU32,

    /// Total number of nodes in the graph
    pub node_count: AtomicU32,

    /// Maximum layer in the graph
    pub max_layer: AtomicU32,

    /// Number of connections per node (M parameter)
    pub m: AtomicU32,

    /// Construction ef parameter
    pub ef_construction: AtomicU32,

    /// Total memory used by the index (bytes)
    pub memory_bytes: AtomicUsize,

    /// Lock for exclusive operations (insertions)
    /// This would map to PostgreSQL's LWLock in actual implementation
    pub lock_exclusive: AtomicU32,

    /// Lock for shared operations (searches)
    pub lock_shared: AtomicU32,

    /// Version counter (incremented on modifications)
    pub version: AtomicU32,

    /// Flags for index state
    pub flags: AtomicU32,
}

impl HnswSharedMem {
    /// Create a new shared memory segment
    pub fn new(m: u32, ef_construction: u32) -> Self {
        Self {
            entry_point: AtomicU32::new(u32::MAX), // Invalid entry point
            node_count: AtomicU32::new(0),
            max_layer: AtomicU32::new(0),
            m: AtomicU32::new(m),
            ef_construction: AtomicU32::new(ef_construction),
            memory_bytes: AtomicUsize::new(0),
            lock_exclusive: AtomicU32::new(0),
            lock_shared: AtomicU32::new(0),
            version: AtomicU32::new(0),
            flags: AtomicU32::new(0),
        }
    }

    /// Try to acquire exclusive lock
    pub fn try_lock_exclusive(&self) -> bool {
        self.lock_exclusive
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    /// Release exclusive lock
    pub fn unlock_exclusive(&self) {
        self.lock_exclusive.store(0, Ordering::Release);
    }

    /// Increment shared lock count
    pub fn lock_shared(&self) {
        self.lock_shared.fetch_add(1, Ordering::Acquire);
    }

    /// Decrement shared lock count
    pub fn unlock_shared(&self) {
        self.lock_shared.fetch_sub(1, Ordering::Release);
    }

    /// Check if exclusively locked
    pub fn is_locked_exclusive(&self) -> bool {
        self.lock_exclusive.load(Ordering::Relaxed) != 0
    }

    /// Get shared lock count
    pub fn shared_lock_count(&self) -> u32 {
        self.lock_shared.load(Ordering::Relaxed)
    }

    /// Increment version (called after modifications)
    pub fn increment_version(&self) -> u32 {
        self.version.fetch_add(1, Ordering::Release)
    }

    /// Get current version
    pub fn version(&self) -> u32 {
        self.version.load(Ordering::Acquire)
    }
}

/// Shared memory segment for IVFFlat index
#[repr(C, align(64))]
pub struct IvfFlatSharedMem {
    /// Number of lists (centroids)
    pub nlists: AtomicU32,

    /// Number of dimensions
    pub dimensions: AtomicU32,

    /// Total number of vectors indexed
    pub vector_count: AtomicU32,

    /// Memory used by the index (bytes)
    pub memory_bytes: AtomicUsize,

    /// Lock for exclusive operations
    pub lock_exclusive: AtomicU32,

    /// Lock for shared operations
    pub lock_shared: AtomicU32,

    /// Version counter
    pub version: AtomicU32,

    /// Flags
    pub flags: AtomicU32,
}

impl IvfFlatSharedMem {
    /// Create a new shared memory segment
    pub fn new(nlists: u32, dimensions: u32) -> Self {
        Self {
            nlists: AtomicU32::new(nlists),
            dimensions: AtomicU32::new(dimensions),
            vector_count: AtomicU32::new(0),
            memory_bytes: AtomicUsize::new(0),
            lock_exclusive: AtomicU32::new(0),
            lock_shared: AtomicU32::new(0),
            version: AtomicU32::new(0),
            flags: AtomicU32::new(0),
        }
    }

    /// Try to acquire exclusive lock
    pub fn try_lock_exclusive(&self) -> bool {
        self.lock_exclusive
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    /// Release exclusive lock
    pub fn unlock_exclusive(&self) {
        self.lock_exclusive.store(0, Ordering::Release);
    }

    /// Increment shared lock count
    pub fn lock_shared(&self) {
        self.lock_shared.fetch_add(1, Ordering::Acquire);
    }

    /// Decrement shared lock count
    pub fn unlock_shared(&self) {
        self.lock_shared.fetch_sub(1, Ordering::Release);
    }
}

// ============================================================================
// TOAST Handling for Large Vectors
// ============================================================================

/// TOAST storage strategy for vectors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastStrategy {
    /// Store inline (no TOAST) - for vectors < 2KB
    Inline,

    /// TOAST with compression - for compressible vectors
    Compressed,

    /// TOAST external storage - for large vectors
    External,

    /// Extended external storage with compression
    ExtendedCompressed,
}

impl ToastStrategy {
    /// Determine optimal TOAST strategy for a vector
    pub fn for_vector(dims: usize, compressibility: f32) -> Self {
        let size = dims * std::mem::size_of::<f32>();

        if size < INLINE_THRESHOLD {
            // Small vectors: always inline
            Self::Inline
        } else if size < TOAST_THRESHOLD {
            // Medium vectors: inline if fits, compress if compressible
            if compressibility > 0.3 {
                Self::Compressed
            } else {
                Self::Inline
            }
        } else if size < 8192 {
            // Large vectors: compress if compressible, else external
            if compressibility > 0.2 {
                Self::Compressed
            } else {
                Self::External
            }
        } else {
            // Very large vectors: always external with compression if beneficial
            if compressibility > 0.15 {
                Self::ExtendedCompressed
            } else {
                Self::External
            }
        }
    }
}

/// Estimate compressibility of vector data
///
/// Returns a value between 0.0 (not compressible) and 1.0 (highly compressible)
pub fn estimate_compressibility(data: &[f32]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }

    let mut zero_count = 0;
    let mut repeated_count = 0;
    let mut prev = f32::NAN;

    for &val in data {
        if val == 0.0 {
            zero_count += 1;
        }
        if val == prev {
            repeated_count += 1;
        }
        prev = val;
    }

    // Simple heuristic: ratio of zeros and repeated values
    let zero_ratio = zero_count as f32 / data.len() as f32;
    let repeat_ratio = repeated_count as f32 / data.len() as f32;

    (zero_ratio * 0.7 + repeat_ratio * 0.3).min(1.0)
}

/// Vector storage descriptor
///
/// Describes how a vector is stored in PostgreSQL (inline or TOASTed)
#[derive(Debug, Clone)]
pub struct VectorStorage {
    /// Storage strategy used
    pub strategy: ToastStrategy,

    /// Original size in bytes
    pub original_size: usize,

    /// Stored size in bytes (after compression if applicable)
    pub stored_size: usize,

    /// Whether data is compressed
    pub compressed: bool,

    /// Whether data is external
    pub external: bool,
}

impl VectorStorage {
    /// Create storage descriptor for inline storage
    pub fn inline(size: usize) -> Self {
        Self {
            strategy: ToastStrategy::Inline,
            original_size: size,
            stored_size: size,
            compressed: false,
            external: false,
        }
    }

    /// Create storage descriptor for compressed storage
    pub fn compressed(original_size: usize, compressed_size: usize) -> Self {
        Self {
            strategy: ToastStrategy::Compressed,
            original_size,
            stored_size: compressed_size,
            compressed: true,
            external: false,
        }
    }

    /// Create storage descriptor for external storage
    pub fn external(size: usize) -> Self {
        Self {
            strategy: ToastStrategy::External,
            original_size: size,
            stored_size: size,
            compressed: false,
            external: true,
        }
    }

    /// Get compression ratio
    pub fn compression_ratio(&self) -> f32 {
        if self.original_size == 0 {
            return 1.0;
        }
        self.stored_size as f32 / self.original_size as f32
    }

    /// Get space savings in bytes
    pub fn space_saved(&self) -> usize {
        self.original_size.saturating_sub(self.stored_size)
    }
}

// ============================================================================
// Memory Statistics
// ============================================================================

/// Get global memory context statistics
pub fn get_memory_stats() -> MemoryStats {
    MemoryStats {
        current_bytes: GLOBAL_VECTOR_CONTEXT.current_bytes(),
        peak_bytes: GLOBAL_VECTOR_CONTEXT.peak_bytes(),
        vector_count: GLOBAL_VECTOR_CONTEXT.count(),
        cache_bytes: VECTOR_CACHE_BYTES.load(Ordering::Relaxed),
    }
}

/// Memory statistics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    /// Current allocated bytes
    pub current_bytes: usize,

    /// Peak allocated bytes
    pub peak_bytes: usize,

    /// Number of vectors
    pub vector_count: u32,

    /// Cache memory bytes
    pub cache_bytes: usize,
}

impl MemoryStats {
    /// Get current memory usage in MB
    pub fn current_mb(&self) -> f64 {
        self.current_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Get peak memory usage in MB
    pub fn peak_mb(&self) -> f64 {
        self.peak_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Get cache memory usage in MB
    pub fn cache_mb(&self) -> f64 {
        self.cache_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Get total memory usage in MB
    pub fn total_mb(&self) -> f64 {
        (self.current_bytes + self.cache_bytes) as f64 / (1024.0 * 1024.0)
    }
}

// ============================================================================
// SQL Functions for Memory Management
// ============================================================================

/// Get detailed memory statistics
#[pg_extern]
fn ruvector_memory_detailed() -> pgrx::JsonB {
    let stats = get_memory_stats();
    pgrx::JsonB(serde_json::json!({
        "current_mb": stats.current_mb(),
        "peak_mb": stats.peak_mb(),
        "cache_mb": stats.cache_mb(),
        "total_mb": stats.total_mb(),
        "vector_count": stats.vector_count,
        "current_bytes": stats.current_bytes,
        "peak_bytes": stats.peak_bytes,
        "cache_bytes": stats.cache_bytes,
    }))
}

/// Reset peak memory tracking
#[pg_extern]
fn ruvector_reset_peak_memory() {
    GLOBAL_VECTOR_CONTEXT
        .peak_bytes
        .store(GLOBAL_VECTOR_CONTEXT.current_bytes(), Ordering::Relaxed);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_header() {
        let header = VectorHeader::new(128, 512);
        assert_eq!(header.dimensions, 128);
        assert_eq!(header.data_size(), 512);
    }

    #[test]
    fn test_hnsw_shared_mem() {
        let shmem = HnswSharedMem::new(16, 64);
        assert_eq!(shmem.m.load(Ordering::Relaxed), 16);
        assert_eq!(shmem.ef_construction.load(Ordering::Relaxed), 64);

        // Test locking
        assert!(shmem.try_lock_exclusive());
        assert!(!shmem.try_lock_exclusive()); // Already locked
        shmem.unlock_exclusive();
        assert!(shmem.try_lock_exclusive()); // Can lock again
    }

    #[test]
    fn test_toast_strategy() {
        // Small vector: inline
        let strategy = ToastStrategy::for_vector(64, 0.0);
        assert_eq!(strategy, ToastStrategy::Inline);

        // Large compressible vector: compressed
        let strategy = ToastStrategy::for_vector(1024, 0.5);
        assert_eq!(strategy, ToastStrategy::Compressed);

        // Large incompressible vector: external
        let strategy = ToastStrategy::for_vector(1024, 0.0);
        assert_eq!(strategy, ToastStrategy::External);
    }

    #[test]
    fn test_compressibility() {
        // Highly compressible (many zeros)
        let data = vec![0.0; 100];
        let comp = estimate_compressibility(&data);
        assert!(comp > 0.6);

        // Not compressible (random values)
        let data: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let comp = estimate_compressibility(&data);
        assert!(comp < 0.3);
    }

    #[test]
    fn test_vector_storage() {
        let storage = VectorStorage::compressed(1000, 400);
        assert_eq!(storage.compression_ratio(), 0.4);
        assert_eq!(storage.space_saved(), 600);
    }

    #[test]
    fn test_memory_context() {
        let ctx = PgVectorContext::new();

        ctx.track_alloc(1024);
        assert_eq!(ctx.current_bytes(), 1024);
        assert_eq!(ctx.count(), 1);

        ctx.track_alloc(512);
        assert_eq!(ctx.current_bytes(), 1536);
        assert_eq!(ctx.peak_bytes(), 1536);

        ctx.track_dealloc(1024);
        assert_eq!(ctx.current_bytes(), 512);
        assert_eq!(ctx.peak_bytes(), 1536); // Peak stays
    }
}
