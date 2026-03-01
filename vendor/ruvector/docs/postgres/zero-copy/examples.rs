// Example code demonstrating zero-copy memory optimization in ruvector-postgres
// This file is for documentation purposes and shows how to use the new APIs

use ruvector_postgres::types::{
    RuVector, VectorData, HnswSharedMem, IvfFlatSharedMem,
    ToastStrategy, estimate_compressibility, get_memory_stats,
    palloc_vector, palloc_vector_aligned, pfree_vector,
    VectorStorage, MemoryStats, PgVectorContext,
};
use std::sync::atomic::Ordering;

// ============================================================================
// Example 1: Zero-Copy Vector Access
// ============================================================================

fn example_zero_copy_access() {
    let vec = RuVector::from_slice(&[1.0, 2.0, 3.0, 4.0]);

    // Zero-copy access to underlying data
    unsafe {
        let ptr = vec.data_ptr();
        let dims = vec.dimensions();

        // Can pass directly to SIMD functions
        // simd_euclidean_distance(ptr, other_ptr, dims);
        println!("Vector pointer: {:?}, dimensions: {}", ptr, dims);
    }

    // Check SIMD alignment
    if vec.is_simd_aligned() {
        println!("Vector is aligned for AVX-512 operations");
    }

    // Get slice without copying
    let slice = vec.as_slice();
    println!("Vector data: {:?}", slice);
}

// ============================================================================
// Example 2: PostgreSQL Memory Context
// ============================================================================

unsafe fn example_pg_memory_context() {
    // Allocate in PostgreSQL memory context
    let dims = 1536;
    let ptr = palloc_vector_aligned(dims);

    // Memory is automatically freed when transaction ends
    // No need for manual cleanup!

    // For manual cleanup (if needed before transaction end):
    // pfree_vector(ptr, dims);

    println!("Allocated {} dimensions at {:?}", dims, ptr);
}

// ============================================================================
// Example 3: Shared Memory Index Access
// ============================================================================

fn example_hnsw_shared_memory() {
    let shmem = HnswSharedMem::new(16, 64);

    // Multiple backends can read concurrently
    shmem.lock_shared();
    let entry_point = shmem.entry_point.load(Ordering::Acquire);
    let node_count = shmem.node_count.load(Ordering::Relaxed);
    println!("HNSW: entry={}, nodes={}", entry_point, node_count);
    shmem.unlock_shared();

    // Exclusive write access
    if shmem.try_lock_exclusive() {
        // Perform insertion
        shmem.node_count.fetch_add(1, Ordering::Relaxed);
        shmem.entry_point.store(42, Ordering::Release);

        // Increment version for MVCC
        let new_version = shmem.increment_version();
        println!("Updated to version {}", new_version);

        shmem.unlock_exclusive();
    }

    // Check locking state
    println!("Locked: {}, Readers: {}",
             shmem.is_locked_exclusive(),
             shmem.shared_lock_count());
}

// ============================================================================
// Example 4: IVFFlat Shared Memory
// ============================================================================

fn example_ivfflat_shared_memory() {
    let shmem = IvfFlatSharedMem::new(100, 1536);

    // Read cluster configuration
    shmem.lock_shared();
    let nlists = shmem.nlists.load(Ordering::Relaxed);
    let dims = shmem.dimensions.load(Ordering::Relaxed);
    println!("IVFFlat: {} lists, {} dims", nlists, dims);
    shmem.unlock_shared();

    // Update vector count after insertion
    if shmem.try_lock_exclusive() {
        shmem.vector_count.fetch_add(1, Ordering::Relaxed);
        shmem.unlock_exclusive();
    }
}

// ============================================================================
// Example 5: TOAST Strategy Selection
// ============================================================================

fn example_toast_strategy() {
    // Small vector: inline storage
    let small_vec = vec![1.0; 64];
    let comp = estimate_compressibility(&small_vec);
    let strategy = ToastStrategy::for_vector(64, comp);
    println!("Small vector (64-d): {:?}", strategy);

    // Large sparse vector: compression beneficial
    let mut sparse = vec![0.0; 10000];
    sparse[100] = 1.0;
    sparse[500] = 2.0;
    let comp = estimate_compressibility(&sparse);
    let strategy = ToastStrategy::for_vector(10000, comp);
    println!("Sparse vector (10K-d): {:?}, compressibility: {:.2}", strategy, comp);

    // Large dense vector: external storage
    let dense = vec![1.0; 10000];
    let comp = estimate_compressibility(&dense);
    let strategy = ToastStrategy::for_vector(10000, comp);
    println!("Dense vector (10K-d): {:?}, compressibility: {:.2}", strategy, comp);
}

// ============================================================================
// Example 6: Compressibility Estimation
// ============================================================================

fn example_compressibility_estimation() {
    // Highly compressible (all zeros)
    let zeros = vec![0.0; 1000];
    let comp = estimate_compressibility(&zeros);
    println!("All zeros: compressibility = {:.2}", comp);

    // Sparse vector
    let mut sparse = vec![0.0; 1000];
    for i in (0..1000).step_by(100) {
        sparse[i] = i as f32;
    }
    let comp = estimate_compressibility(&sparse);
    println!("Sparse (10% nnz): compressibility = {:.2}", comp);

    // Dense random
    let random: Vec<f32> = (0..1000).map(|i| (i as f32) * 0.123).collect();
    let comp = estimate_compressibility(&random);
    println!("Dense random: compressibility = {:.2}", comp);

    // Repeated values
    let repeated = vec![1.0; 1000];
    let comp = estimate_compressibility(&repeated);
    println!("Repeated values: compressibility = {:.2}", comp);
}

// ============================================================================
// Example 7: Vector Storage Tracking
// ============================================================================

fn example_vector_storage() {
    // Inline storage
    let inline_storage = VectorStorage::inline(512);
    println!("Inline: {} bytes", inline_storage.stored_size);

    // Compressed storage
    let compressed_storage = VectorStorage::compressed(10000, 2000);
    println!("Compressed: {} → {} bytes ({:.1}% compression)",
             compressed_storage.original_size,
             compressed_storage.stored_size,
             (1.0 - compressed_storage.compression_ratio()) * 100.0);
    println!("Space saved: {} bytes", compressed_storage.space_saved());

    // External storage
    let external_storage = VectorStorage::external(40000);
    println!("External: {} bytes (stored in TOAST table)",
             external_storage.stored_size);
}

// ============================================================================
// Example 8: Memory Statistics Tracking
// ============================================================================

fn example_memory_statistics() {
    let stats = get_memory_stats();

    println!("Current memory: {:.2} MB", stats.current_mb());
    println!("Peak memory: {:.2} MB", stats.peak_mb());
    println!("Cache memory: {:.2} MB", stats.cache_mb());
    println!("Total memory: {:.2} MB", stats.total_mb());
    println!("Vector count: {}", stats.vector_count);

    // Detailed breakdown
    println!("\nDetailed breakdown:");
    println!("  Current: {} bytes", stats.current_bytes);
    println!("  Peak: {} bytes", stats.peak_bytes);
    println!("  Cache: {} bytes", stats.cache_bytes);
}

// ============================================================================
// Example 9: Memory Context Tracking
// ============================================================================

fn example_memory_context_tracking() {
    let ctx = PgVectorContext::new();

    // Simulate allocations
    ctx.track_alloc(1024);
    println!("After 1KB alloc: {} bytes, {} vectors",
             ctx.current_bytes(), ctx.count());

    ctx.track_alloc(2048);
    println!("After 2KB alloc: {} bytes, {} vectors",
             ctx.current_bytes(), ctx.count());

    println!("Peak usage: {} bytes", ctx.peak_bytes());

    // Simulate deallocation
    ctx.track_dealloc(1024);
    println!("After 1KB free: {} bytes (peak: {})",
             ctx.current_bytes(), ctx.peak_bytes());
}

// ============================================================================
// Example 10: Production Usage Pattern
// ============================================================================

fn example_production_usage() {
    // Typical production workflow

    // 1. Create vector
    let embedding = RuVector::from_slice(&vec![0.1; 1536]);

    // 2. Check storage requirements
    let data = embedding.as_slice();
    let compressibility = estimate_compressibility(data);
    let strategy = ToastStrategy::for_vector(embedding.dimensions(), compressibility);

    println!("Storage strategy: {:?}", strategy);

    // 3. Initialize shared memory index
    let hnsw_shmem = HnswSharedMem::new(16, 64);

    // 4. Insert with locking
    if hnsw_shmem.try_lock_exclusive() {
        // Perform insertion
        let new_node_id = 12345; // Simulated insertion

        hnsw_shmem.node_count.fetch_add(1, Ordering::Relaxed);
        hnsw_shmem.entry_point.store(new_node_id, Ordering::Release);
        hnsw_shmem.increment_version();

        hnsw_shmem.unlock_exclusive();
    }

    // 5. Search with concurrent access
    hnsw_shmem.lock_shared();
    let entry = hnsw_shmem.entry_point.load(Ordering::Acquire);
    println!("Search starting from node {}", entry);
    hnsw_shmem.unlock_shared();

    // 6. Monitor memory
    let stats = get_memory_stats();
    if stats.current_mb() > 1000.0 {
        println!("WARNING: High memory usage: {:.2} MB", stats.current_mb());
    }
}

// ============================================================================
// Example 11: SIMD-Aligned Operations
// ============================================================================

fn example_simd_aligned_operations() {
    // Create vectors with different alignment
    let vec1 = RuVector::from_slice(&vec![1.0; 1536]);

    unsafe {
        // Check alignment
        if vec1.is_simd_aligned() {
            let ptr = vec1.data_ptr();
            println!("Vector is aligned for AVX-512");

            // Can use aligned SIMD loads
            // let result = _mm512_load_ps(ptr);
        } else {
            let ptr = vec1.data_ptr();
            println!("Vector requires unaligned loads");

            // Use unaligned SIMD loads
            // let result = _mm512_loadu_ps(ptr);
        }
    }

    // Check memory layout
    println!("Memory size: {} bytes", vec1.memory_size());
    println!("Data size: {} bytes", vec1.data_size());
    println!("Is inline: {}", vec1.is_inline());
}

// ============================================================================
// Example 12: Concurrent Index Operations
// ============================================================================

fn example_concurrent_operations() {
    let shmem = HnswSharedMem::new(16, 64);

    // Simulate multiple concurrent readers
    println!("Concurrent reads:");
    for i in 0..5 {
        shmem.lock_shared();
        let entry = shmem.entry_point.load(Ordering::Acquire);
        println!("  Reader {}: entry_point = {}", i, entry);
        shmem.unlock_shared();
    }

    // Single writer
    println!("\nExclusive write:");
    if shmem.try_lock_exclusive() {
        println!("  Acquired exclusive lock");
        shmem.entry_point.store(999, Ordering::Release);
        let version = shmem.increment_version();
        println!("  Updated to version {}", version);
        shmem.unlock_exclusive();
        println!("  Released exclusive lock");
    }

    // Verify update
    shmem.lock_shared();
    let entry = shmem.entry_point.load(Ordering::Acquire);
    let version = shmem.version();
    println!("\nAfter update: entry={}, version={}", entry, version);
    shmem.unlock_shared();
}

// ============================================================================
// Main function (for demonstration)
// ============================================================================

#[cfg(test)]
mod examples {
    use super::*;

    #[test]
    fn run_all_examples() {
        println!("\n=== Example 1: Zero-Copy Vector Access ===");
        example_zero_copy_access();

        // Skip unsafe examples in tests
        // unsafe { example_pg_memory_context(); }

        println!("\n=== Example 3: HNSW Shared Memory ===");
        example_hnsw_shared_memory();

        println!("\n=== Example 4: IVFFlat Shared Memory ===");
        example_ivfflat_shared_memory();

        println!("\n=== Example 5: TOAST Strategy ===");
        example_toast_strategy();

        println!("\n=== Example 6: Compressibility ===");
        example_compressibility_estimation();

        println!("\n=== Example 7: Vector Storage ===");
        example_vector_storage();

        println!("\n=== Example 8: Memory Statistics ===");
        example_memory_statistics();

        println!("\n=== Example 9: Memory Context ===");
        example_memory_context_tracking();

        println!("\n=== Example 10: Production Usage ===");
        example_production_usage();

        println!("\n=== Example 11: SIMD Alignment ===");
        example_simd_aligned_operations();

        println!("\n=== Example 12: Concurrent Operations ===");
        example_concurrent_operations();
    }
}
