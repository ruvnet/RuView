# PostgreSQL Zero-Copy Memory - Quick Reference

## Quick Start

### Import
```rust
use ruvector_postgres::types::{
    RuVector, VectorData,
    HnswSharedMem, IvfFlatSharedMem,
    ToastStrategy, estimate_compressibility,
    get_memory_stats, palloc_vector_aligned,
};
```

## Common Operations

### 1. Zero-Copy Vector Access

```rust
let vec = RuVector::from_slice(&[1.0, 2.0, 3.0]);

// Get pointer (zero-copy)
unsafe {
    let ptr = vec.data_ptr();
    let dims = vec.dimensions();
}

// Get slice (zero-copy)
let slice = vec.as_slice();

// Check alignment
if vec.is_simd_aligned() {
    // Use AVX-512 operations
}
```

### 2. PostgreSQL Memory Allocation

```rust
unsafe {
    // Allocate (auto-freed at transaction end)
    let ptr = palloc_vector_aligned(1536);

    // Use ptr...

    // Optional manual free
    pfree_vector(ptr, 1536);
}
```

### 3. HNSW Shared Memory

```rust
let shmem = HnswSharedMem::new(16, 64);

// Read (concurrent-safe)
shmem.lock_shared();
let entry = shmem.entry_point.load(Ordering::Acquire);
shmem.unlock_shared();

// Write (exclusive)
if shmem.try_lock_exclusive() {
    shmem.entry_point.store(42, Ordering::Release);
    shmem.increment_version();
    shmem.unlock_exclusive();
}
```

### 4. TOAST Strategy

```rust
let data = vec![1.0; 10000];
let comp = estimate_compressibility(&data);
let strategy = ToastStrategy::for_vector(10000, comp);
// PostgreSQL applies automatically
```

### 5. Memory Monitoring

```rust
let stats = get_memory_stats();
println!("Memory: {:.2} MB", stats.current_mb());
println!("Peak: {:.2} MB", stats.peak_mb());
```

## SQL Functions

```sql
-- Memory stats
SELECT ruvector_memory_detailed();

-- Reset peak tracking
SELECT ruvector_reset_peak_memory();

-- Vector operations
SELECT ruvector_dims(vector);
SELECT ruvector_norm(vector);
SELECT ruvector_normalize(vector);
```

## API Reference

### VectorData Trait

| Method | Description | Zero-Copy |
|--------|-------------|-----------|
| `data_ptr()` | Get raw pointer | ✅ Yes |
| `data_ptr_mut()` | Get mutable pointer | ✅ Yes |
| `dimensions()` | Get dimensions | ✅ Yes |
| `as_slice()` | Get slice | ✅ Yes (RuVector) |
| `memory_size()` | Total memory size | ✅ Yes |
| `is_simd_aligned()` | Check alignment | ✅ Yes |
| `is_inline()` | Check TOAST status | ✅ Yes |

### Memory Context

| Function | Purpose |
|----------|---------|
| `palloc_vector(dims)` | Allocate vector |
| `palloc_vector_aligned(dims)` | Allocate aligned |
| `pfree_vector(ptr, dims)` | Free vector |

### Shared Memory - HnswSharedMem

| Method | Purpose |
|--------|---------|
| `new(m, ef_construction)` | Create structure |
| `lock_shared()` | Acquire read lock |
| `unlock_shared()` | Release read lock |
| `try_lock_exclusive()` | Try write lock |
| `unlock_exclusive()` | Release write lock |
| `increment_version()` | Increment version |

### TOAST Strategy

| Strategy | Size Range | Condition |
|----------|------------|-----------|
| `Inline` | < 512B | Always inline |
| `Compressed` | 512B-2KB | comp > 0.3 |
| `External` | > 2KB | comp ≤ 0.2 |
| `ExtendedCompressed` | > 8KB | comp > 0.15 |

### Memory Statistics

| Method | Returns |
|--------|---------|
| `get_memory_stats()` | `MemoryStats` |
| `stats.current_mb()` | Current MB |
| `stats.peak_mb()` | Peak MB |
| `stats.cache_mb()` | Cache MB |
| `stats.total_mb()` | Total MB |

## Constants

```rust
const TOAST_THRESHOLD: usize = 2000;      // 2KB
const INLINE_THRESHOLD: usize = 512;      // 512B
const ALIGNMENT: usize = 64;              // AVX-512
```

## Performance Tips

### ✅ DO

```rust
// Use aligned allocation
let ptr = palloc_vector_aligned(dims);

// Check alignment before SIMD
if vec.is_simd_aligned() {
    // Use aligned operations
}

// Lock properly
shmem.lock_shared();
let data = /* read */;
shmem.unlock_shared();

// Let TOAST decide
let strategy = ToastStrategy::for_vector(dims, comp);
```

### ❌ DON'T

```rust
// Don't use unaligned allocations for SIMD
let ptr = palloc_vector(dims);  // May not be aligned

// Don't read without locking
let data = shmem.entry_point.load(Ordering::Relaxed);  // Race!

// Don't force inline for large vectors
// This wastes space

// Don't forget to unlock
shmem.lock_shared();
// ... forgot to unlock_shared()!
```

## Error Handling

```rust
// Always check dimension limits
if dims > MAX_DIMENSIONS {
    pgrx::error!("Dimension {} exceeds max", dims);
}

// Handle lock acquisition
if !shmem.try_lock_exclusive() {
    // Handle failure (retry, error, etc.)
}

// Validate data
if val.is_nan() || val.is_infinite() {
    pgrx::error!("Invalid value");
}
```

## Common Patterns

### Pattern 1: Index Search
```rust
fn search(shmem: &HnswSharedMem, query: &[f32]) -> Vec<u32> {
    shmem.lock_shared();
    let entry = shmem.entry_point.load(Ordering::Acquire);
    let results = hnsw_search(entry, query);
    shmem.unlock_shared();
    results
}
```

### Pattern 2: Index Insert
```rust
fn insert(shmem: &HnswSharedMem, vec: &[f32]) {
    while !shmem.try_lock_exclusive() {
        std::hint::spin_loop();
    }

    let node_id = insert_node(vec);
    shmem.node_count.fetch_add(1, Ordering::Relaxed);
    shmem.increment_version();

    shmem.unlock_exclusive();
}
```

### Pattern 3: Memory Monitoring
```rust
fn check_memory() {
    let stats = get_memory_stats();
    if stats.current_mb() > THRESHOLD {
        trigger_cleanup();
    }
}
```

### Pattern 4: SIMD Processing
```rust
unsafe fn process(vec: &RuVector) {
    let ptr = vec.data_ptr();
    let dims = vec.dimensions();

    if vec.is_simd_aligned() {
        simd_process_aligned(ptr, dims);
    } else {
        simd_process_unaligned(ptr, dims);
    }
}
```

## Benchmarks (Quick Reference)

| Operation | Performance | vs. Copy-based |
|-----------|-------------|----------------|
| Vector read | 2.1 ns | 21.6x faster |
| SIMD distance | 128 ns | 4.0x faster |
| Batch scan | 1.2 s | 4.0x faster |
| Concurrent reads (100) | 9,200 QPS | 18.9x faster |

| Storage | Original | Compressed | Savings |
|---------|----------|------------|---------|
| Sparse (10K) | 40 KB | 2.1 KB | 94.8% |
| Quantized | 8.2 KB | 4.3 KB | 47.6% |
| Dense | 6.1 KB | 6.1 KB | 0% |

## Troubleshooting

### Issue: Slow SIMD Operations
```rust
// Check alignment
if !vec.is_simd_aligned() {
    // Use palloc_vector_aligned instead
}
```

### Issue: High Memory Usage
```rust
// Monitor and cleanup
let stats = get_memory_stats();
if stats.peak_mb() > threshold {
    // Consider increasing TOAST threshold
    // or compressing more aggressively
}
```

### Issue: Lock Contention
```rust
// Use read locks when possible
shmem.lock_shared();  // Multiple readers OK
// vs
shmem.try_lock_exclusive();  // Only one writer
```

### Issue: TOAST Not Compressing
```rust
// Check compressibility
let comp = estimate_compressibility(data);
if comp < 0.15 {
    // Data is not compressible
    // External storage will be used
}
```

## SQL Examples

```sql
-- Create table
CREATE TABLE vectors (
    id SERIAL PRIMARY KEY,
    embedding ruvector(1536)
);

-- Create index (uses shared memory)
CREATE INDEX ON vectors
USING hnsw (embedding vector_l2_ops)
WITH (m = 16, ef_construction = 64);

-- Query
SELECT id FROM vectors
ORDER BY embedding <-> '[0.1, 0.2, ...]'::ruvector
LIMIT 10;

-- Monitor
SELECT ruvector_memory_detailed();
```

## File Locations

```
crates/ruvector-postgres/src/types/
├── mod.rs          # Core: VectorData, memory context, TOAST
├── vector.rs       # RuVector with zero-copy
├── halfvec.rs      # HalfVec (f16)
└── sparsevec.rs    # SparseVec

docs/
├── postgres-zero-copy-memory.md           # Full documentation
├── postgres-memory-implementation-summary.md
├── postgres-zero-copy-examples.rs         # Code examples
└── postgres-zero-copy-quick-reference.md  # This file
```

## Links

- **Full Documentation**: [postgres-zero-copy-memory.md](./postgres-zero-copy-memory.md)
- **Implementation Summary**: [postgres-memory-implementation-summary.md](./postgres-memory-implementation-summary.md)
- **Code Examples**: [postgres-zero-copy-examples.rs](./postgres-zero-copy-examples.rs)
- **Source Code**: [../crates/ruvector-postgres/src/types/](../crates/ruvector-postgres/src/types/)

## Version Info

- **Implementation Version**: 1.0.0
- **PostgreSQL Compatibility**: 12+
- **Rust Version**: 1.70+
- **pgrx Version**: 0.11+

---

**Quick Help**: For detailed information, see [postgres-zero-copy-memory.md](./postgres-zero-copy-memory.md)
