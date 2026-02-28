# RuVector-Postgres Architecture

## Overview

RuVector-Postgres is a high-performance, drop-in replacement for the pgvector extension, built in Rust using the pgrx framework. It provides SIMD-optimized vector similarity search with advanced indexing algorithms, quantization support, and hybrid search capabilities.

## Design Goals

1. **pgvector API Compatibility**: 100% compatible SQL interface with pgvector
2. **Superior Performance**: 2-10x faster than pgvector through SIMD and algorithmic optimizations
3. **Memory Efficiency**: Up to 32x memory reduction via quantization
4. **Neon Compatibility**: Designed for serverless PostgreSQL (Neon, Supabase, etc.)
5. **Production Ready**: Battle-tested algorithms from ruvector-core

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           PostgreSQL Server                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐ │
│  │                      RuVector-Postgres Extension                         │ │
│  ├─────────────────────────────────────────────────────────────────────────┤ │
│  │                                                                           │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │ │
│  │  │   Vector    │  │   HNSW      │  │  IVFFlat    │  │   Flat Index    │  │ │
│  │  │   Type      │  │   Index     │  │   Index     │  │   (fallback)    │  │ │
│  │  │             │  │             │  │             │  │                 │  │ │
│  │  │ - ruvector  │  │ - O(log n)  │  │ - O(√n)     │  │ - O(n)          │  │ │
│  │  │ - halfvec   │  │ - 95%+ rec  │  │ - clusters  │  │ - exact search  │  │ │
│  │  │ - sparsevec │  │ - SIMD ops  │  │ - training  │  │                 │  │ │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └────────┬────────┘  │ │
│  │         │                │                │                   │           │ │
│  │  ┌──────┴────────────────┴────────────────┴───────────────────┴────────┐  │ │
│  │  │                     SIMD Distance Layer                              │  │ │
│  │  │                                                                       │  │ │
│  │  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────────┐  │  │ │
│  │  │  │  AVX-512   │  │   AVX2     │  │   NEON     │  │   Scalar       │  │  │ │
│  │  │  │  (x86_64)  │  │  (x86_64)  │  │  (ARM64)   │  │   Fallback     │  │  │ │
│  │  │  └────────────┘  └────────────┘  └────────────┘  └────────────────┘  │  │ │
│  │  └──────────────────────────────────────────────────────────────────────┘  │ │
│  │                                                                           │ │
│  │  ┌──────────────────────────────────────────────────────────────────────┐  │ │
│  │  │                    Quantization Engine                                │  │ │
│  │  │                                                                       │  │ │
│  │  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────────┐  │  │ │
│  │  │  │   Scalar   │  │  Product   │  │   Binary   │  │   Half-Prec    │  │  │ │
│  │  │  │    (4x)    │  │   (8-16x)  │  │    (32x)   │  │    (2x)        │  │  │ │
│  │  │  └────────────┘  └────────────┘  └────────────┘  └────────────────┘  │  │ │
│  │  └──────────────────────────────────────────────────────────────────────┘  │ │
│  │                                                                           │ │
│  │  ┌──────────────────────────────────────────────────────────────────────┐  │ │
│  │  │                    Hybrid Search Engine                               │  │ │
│  │  │                                                                       │  │ │
│  │  │  ┌─────────────────────┐  ┌─────────────────────┐  ┌──────────────┐  │  │ │
│  │  │  │  Vector Similarity  │  │   BM25 Text Search  │  │  RRF Fusion  │  │  │ │
│  │  │  │     (dense)         │  │      (sparse)       │  │  (ranking)   │  │  │ │
│  │  │  └─────────────────────┘  └─────────────────────┘  └──────────────┘  │  │ │
│  │  └──────────────────────────────────────────────────────────────────────┘  │ │
│  │                                                                           │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                                                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Vector Types

#### `ruvector` - Primary Vector Type

**Varlena Memory Layout (Zero-Copy Design)**

```
┌─────────────────────────────────────────────────────────────────┐
│                    RuVector Varlena Layout                       │
├─────────────────────────────────────────────────────────────────┤
│  Bytes 0-3    │  Bytes 4-5   │  Bytes 6-7   │  Bytes 8+        │
│  vl_len_      │  dimensions  │  _unused     │  f32 data...     │
│  (varlena hdr)│  (u16)       │  (padding)   │  [dim0, dim1...] │
├─────────────────────────────────────────────────────────────────┤
│  4 bytes      │  2 bytes     │  2 bytes     │  4*dims bytes    │
│  PostgreSQL   │  pgvector    │  Alignment   │  Vector data     │
│  header       │  compatible  │  to 8 bytes  │  (f32 floats)    │
└─────────────────────────────────────────────────────────────────┘
```

**Key Layout Features:**

1. **Varlena Header (VARHDRSZ)**: Standard PostgreSQL variable-length type header (4 bytes)
2. **Dimensions (u16)**: Compatible with pgvector's 16-bit dimension count (max 16,000)
3. **Padding (2 bytes)**: Ensures f32 data is 8-byte aligned for efficient SIMD access
4. **Data Array**: Contiguous f32 elements for zero-copy SIMD operations

**Memory Alignment Requirements:**

- Total header size: 8 bytes (4 + 2 + 2)
- Data alignment: 8-byte aligned for optimal performance
- SIMD alignment:
  - AVX-512 prefers 64-byte alignment (checked at runtime)
  - AVX2 prefers 32-byte alignment (checked at runtime)
  - Unaligned loads used as fallback (minimal performance penalty)

**Zero-Copy Access Pattern:**

```rust
// Direct pointer access to varlena data (zero allocation)
pub unsafe fn as_ptr(&self) -> *const f32 {
    // Skip varlena header (4 bytes) + RuVectorHeader (4 bytes)
    let base = self as *const _ as *const u8;
    base.add(VARHDRSZ + RuVectorHeader::SIZE) as *const f32
}

// SIMD functions operate directly on this pointer
let distance = l2_distance_ptr_avx512(vec_a.as_ptr(), vec_b.as_ptr(), dims);
```

**SQL Usage:**

```sql
-- Dimensions: 1 to 16,000
-- Storage: 4 bytes per dimension (f32) + 8 bytes header
CREATE TABLE items (
    id SERIAL PRIMARY KEY,
    embedding ruvector(1536)  -- OpenAI embedding dimensions
);

-- Total storage per vector: 8 + (1536 * 4) = 6,152 bytes
```

#### `halfvec` - Half-Precision Vector

**Varlena Layout:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    HalfVec Varlena Layout                        │
├─────────────────────────────────────────────────────────────────┤
│  Bytes 0-3    │  Bytes 4-5   │  Bytes 6-7   │  Bytes 8+        │
│  vl_len_      │  dimensions  │  _unused     │  f16 data...     │
│  (varlena hdr)│  (u16)       │  (padding)   │  [dim0, dim1...] │
├─────────────────────────────────────────────────────────────────┤
│  4 bytes      │  2 bytes     │  2 bytes     │  2*dims bytes    │
│  PostgreSQL   │  pgvector    │  Alignment   │  Half-precision  │
│  header       │  compatible  │  to 8 bytes  │  (f16 floats)    │
└─────────────────────────────────────────────────────────────────┘
```

**Storage Benefits:**

- 50% memory savings vs ruvector
- Minimal accuracy loss (<0.01% for most embeddings)
- SIMD f16 support on modern CPUs (AVX-512 FP16, ARM Neon FP16)

```sql
-- Storage: 2 bytes per dimension (f16) + 8 bytes header
-- 50% memory savings, minimal accuracy loss
CREATE TABLE items (
    id SERIAL PRIMARY KEY,
    embedding halfvec(1536)
);

-- Total storage per vector: 8 + (1536 * 2) = 3,080 bytes
```

#### `sparsevec` - Sparse Vector

**Varlena Layout:**

```
┌─────────────────────────────────────────────────────────────────┐
│                  SparseVec Varlena Layout                        │
├─────────────────────────────────────────────────────────────────┤
│  Bytes 0-3    │  Bytes 4-7   │  Bytes 8-11  │  Bytes 12+       │
│  vl_len_      │  dimensions  │  nnz         │  indices+values  │
│  (varlena hdr)│  (u32)       │  (u32)       │  [(idx,val)...]  │
├─────────────────────────────────────────────────────────────────┤
│  4 bytes      │  4 bytes     │  4 bytes     │  8*nnz bytes     │
│  PostgreSQL   │  Total dims  │  Non-zero    │  (u32,f32) pairs │
│  header       │  (full size) │  count       │  for sparse data │
└─────────────────────────────────────────────────────────────────┘
```

**Storage:** Only non-zero elements stored (u32 index + f32 value pairs)

```sql
-- Storage: Only non-zero elements stored
-- Ideal for high-dimensional sparse data (BM25, TF-IDF)
CREATE TABLE items (
    id SERIAL PRIMARY KEY,
    sparse_embedding sparsevec(50000)
);

-- Total storage: 12 + (nnz * 8) bytes
-- Example: 100 non-zero out of 50,000 = 12 + 800 = 812 bytes
```

### 2. Distance Operators

| Operator | Distance Metric | Description | SIMD Optimized |
|----------|----------------|-------------|----------------|
| `<->` | L2 (Euclidean) | `sqrt(sum((a[i] - b[i])^2))` | ✓ |
| `<#>` | Inner Product | `-sum(a[i] * b[i])` (negative for ORDER BY) | ✓ |
| `<=>` | Cosine | `1 - (a·b)/(‖a‖‖b‖)` | ✓ |
| `<+>` | L1 (Manhattan) | `sum(abs(a[i] - b[i]))` | ✓ |
| `<~>` | Hamming | Bit differences (binary vectors) | ✓ |
| `<%>` | Jaccard | Set similarity (sparse vectors) | - |

### 3. SIMD Dispatch Mechanism

**Runtime Feature Detection:**

```rust
/// Initialize SIMD dispatch table at extension load
pub fn init_simd_dispatch() {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx512f") {
            SIMD_LEVEL.store(SimdLevel::AVX512, Ordering::Relaxed);
            return;
        }
        if is_x86_feature_detected!("avx2") {
            SIMD_LEVEL.store(SimdLevel::AVX2, Ordering::Relaxed);
            return;
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if is_aarch64_feature_detected!("neon") {
            SIMD_LEVEL.store(SimdLevel::NEON, Ordering::Relaxed);
            return;
        }
    }

    SIMD_LEVEL.store(SimdLevel::Scalar, Ordering::Relaxed);
}
```

**Dispatch Flow:**

```
┌─────────────────────────────────────────────────────────────────┐
│              Distance Function Call (SQL Operator)               │
├─────────────────────────────────────────────────────────────────┤
│                              ↓                                   │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │    euclidean_distance(a: &[f32], b: &[f32]) -> f32         ││
│  │    ↓                                                         ││
│  │    Check SIMD_LEVEL (atomic read, cached)                   ││
│  └─────────────────────────────────────────────────────────────┘│
│                              ↓                                   │
│         ┌────────────────────┴────────────────────┐             │
│         ↓                                          ↓             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │  AVX-512?    │  │  AVX2?       │  │  NEON/Scalar?        │  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────────────┘  │
│         ↓                  ↓                  ↓                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │ 16 floats/   │  │ 8 floats/    │  │ 4 floats (NEON) or   │  │
│  │ iteration    │  │ iteration    │  │ 1 float (scalar)     │  │
│  │              │  │              │  │                      │  │
│  │ _mm512_*     │  │ _mm256_*     │  │ vaddq_f32/for loop   │  │
│  │ FMA support  │  │ FMA support  │  │                      │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
│         ↓                  ↓                  ↓                  │
│         └────────────────────┬─────────────────┘                │
│                              ↓                                   │
│                    ┌──────────────────┐                         │
│                    │  Return distance │                         │
│                    └──────────────────┘                         │
└─────────────────────────────────────────────────────────────────┘
```

**Performance Characteristics:**

| SIMD Level | Floats/Iter | Relative Speed | Instruction Examples |
|------------|-------------|----------------|---------------------|
| AVX-512 | 16 | 16x | `_mm512_loadu_ps`, `_mm512_fmadd_ps` |
| AVX2 | 8 | 8x | `_mm256_loadu_ps`, `_mm256_fmadd_ps` |
| NEON | 4 | 4x | `vld1q_f32`, `vmlaq_f32` |
| Scalar | 1 | 1x | Standard f32 operations |

### 4. TOAST Handling

**TOAST (The Oversized-Attribute Storage Technique):**

PostgreSQL automatically TOASTs values > ~2KB. RuVector handles this transparently:

```rust
/// Detoast varlena pointer if needed
#[inline]
unsafe fn detoast_vector(raw: *mut varlena) -> *mut varlena {
    if VARATT_IS_EXTENDED(raw) {
        // PostgreSQL automatically detoasts
        pg_detoast_datum(raw as *const varlena) as *mut varlena
    } else {
        raw
    }
}
```

**When TOAST Occurs:**

- RuVector: ~512+ dimensions (2048+ bytes)
- HalfVec: ~1024+ dimensions (2048+ bytes)
- Automatic compression and external storage

**Performance Impact:**

- First access: Detoasting overhead (~10-50μs)
- Subsequent access: Cached in PostgreSQL buffer
- Index operations: Typically work with detoasted values

### 5. Index Types

#### HNSW (Hierarchical Navigable Small World)

```sql
CREATE INDEX ON items USING ruhnsw (embedding ruvector_l2_ops)
WITH (m = 16, ef_construction = 200);
```

**Parameters:**
- `m`: Maximum connections per layer (default: 16, range: 2-100)
- `ef_construction`: Build-time search breadth (default: 64, range: 4-1000)

**Characteristics:**
- Search: O(log n)
- Insert: O(log n)
- Memory: ~1.5x index overhead
- Recall: 95-99%+ with tuned parameters

**HNSW Index Layout:**

```
┌─────────────────────────────────────────────────────────────────┐
│                      HNSW Index Structure                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  Layer L (top):     ○──────○                                     │
│                     │      │                                     │
│  Layer L-1:         ○──○───○──○                                  │
│                     │  │   │  │                                  │
│  Layer L-2:         ○──○───○──○──○──○                            │
│                     │  │   │  │  │  │                            │
│  Layer 0 (base):    ○──○───○──○──○──○──○──○──○                   │
│                                                                   │
│  Entry Point: Top layer node                                     │
│  Search: Greedy descent + local beam search                     │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

#### IVFFlat (Inverted File with Flat Quantization)

```sql
CREATE INDEX ON items USING ruivfflat (embedding ruvector_l2_ops)
WITH (lists = 100);
```

**Parameters:**
- `lists`: Number of clusters (default: sqrt(n), recommended: rows/1000 to rows/10000)

**Characteristics:**
- Search: O(√n)
- Insert: O(1) after training
- Memory: Minimal overhead
- Recall: 90-95% with `probes = sqrt(lists)`

## Query Execution Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                      Query: SELECT ... ORDER BY v <-> q         │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  1. Parse & Plan                                                 │
│     └─> Identify index scan opportunity                         │
│                                                                   │
│  2. Index Selection                                              │
│     └─> Choose HNSW/IVFFlat based on cost estimation            │
│                                                                   │
│  3. Index Scan (SIMD-accelerated)                               │
│     ├─> HNSW: Navigate layers, beam search at layer 0          │
│     └─> IVFFlat: Probe nearest centroids, scan cells           │
│                                                                   │
│  4. Distance Calculation (per candidate)                        │
│     ├─> Detoast vector if needed                               │
│     ├─> Zero-copy pointer access                               │
│     ├─> SIMD dispatch (AVX-512/AVX2/NEON/Scalar)               │
│     └─> Full precision or quantized distance                    │
│                                                                   │
│  5. Result Aggregation                                          │
│     └─> Return top-k with distances                             │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

## Comparison with pgvector

| Feature | pgvector 0.8.0 | RuVector-Postgres |
|---------|---------------|-------------------|
| Vector dimensions | 16,000 max | 16,000 max |
| HNSW index | ✓ | ✓ (optimized) |
| IVFFlat index | ✓ | ✓ (optimized) |
| Half-precision | ✓ | ✓ |
| Sparse vectors | ✓ | ✓ |
| Binary quantization | ✓ | ✓ |
| Product quantization | ✗ | ✓ |
| Scalar quantization | ✗ | ✓ |
| AVX-512 optimized | Partial | Full |
| ARM NEON optimized | ✗ | ✓ |
| Zero-copy access | ✗ | ✓ |
| Varlena alignment | Basic | Optimized (8-byte) |
| Hybrid search | ✗ | ✓ |
| Filtered HNSW | Partial | ✓ |
| Parallel queries | ✓ | ✓ (PARALLEL SAFE) |

## Thread Safety

RuVector-Postgres is fully thread-safe:

- **Read operations**: Lock-free concurrent reads
- **Write operations**: Fine-grained locking per graph layer
- **Index builds**: Parallel with work-stealing

```rust
// Internal synchronization primitives
pub struct HnswIndex {
    layers: Vec<RwLock<Layer>>,           // Per-layer locks
    entry_point: AtomicUsize,             // Lock-free entry point
    node_count: AtomicUsize,              // Lock-free counter
    vectors: DashMap<NodeId, Vec<f32>>,   // Concurrent hashmap
}
```

## Extension Dependencies

```toml
[dependencies]
pgrx = "0.12"                  # PostgreSQL extension framework
simsimd = "5.9"                # SIMD-accelerated distance functions
parking_lot = "0.12"           # Fast synchronization primitives
dashmap = "6.0"                # Concurrent hashmap
rayon = "1.10"                 # Data parallelism
half = "2.4"                   # Half-precision floats
bitflags = "2.6"               # Compact flags storage
```

## Performance Tuning

### Index Build Performance

```sql
-- Parallel index build (uses all available cores)
SET maintenance_work_mem = '8GB';
SET max_parallel_maintenance_workers = 8;

CREATE INDEX CONCURRENTLY ON items
USING ruhnsw (embedding ruvector_l2_ops)
WITH (m = 32, ef_construction = 400);
```

### Search Performance

```sql
-- Adjust search quality vs speed tradeoff
SET ruvector.ef_search = 200;  -- Higher = better recall, slower
SET ruvector.probes = 10;      -- For IVFFlat: more probes = better recall

-- Use iterative scan for filtered queries
SELECT * FROM items
WHERE category = 'electronics'
ORDER BY embedding <-> '[0.1, 0.2, ...]'::ruvector
LIMIT 10;
```

## File Structure

```
crates/ruvector-postgres/
├── Cargo.toml                    # Rust dependencies
├── ruvector.control              # Extension metadata
├── docs/
│   ├── ARCHITECTURE.md           # This file
│   ├── NEON_COMPATIBILITY.md     # Neon deployment guide
│   ├── SIMD_OPTIMIZATION.md      # SIMD implementation details
│   ├── INSTALLATION.md           # Installation instructions
│   ├── API.md                    # SQL API reference
│   └── MIGRATION.md              # Migration from pgvector
├── sql/
│   ├── ruvector--0.1.0.sql       # Extension SQL definitions
│   └── ruvector--0.0.0--0.1.0.sql # Migration script
├── src/
│   ├── lib.rs                    # Extension entry point
│   ├── types/
│   │   ├── mod.rs
│   │   ├── vector.rs             # ruvector type (zero-copy varlena)
│   │   ├── halfvec.rs            # Half-precision vector
│   │   └── sparsevec.rs          # Sparse vector
│   ├── distance/
│   │   ├── mod.rs
│   │   ├── simd.rs               # SIMD implementations (AVX-512/AVX2/NEON)
│   │   └── scalar.rs             # Scalar fallbacks
│   ├── index/
│   │   ├── mod.rs
│   │   ├── hnsw.rs               # HNSW implementation
│   │   ├── ivfflat.rs            # IVFFlat implementation
│   │   └── scan.rs               # Index scan operators
│   ├── quantization/
│   │   ├── mod.rs
│   │   ├── scalar.rs             # SQ8 quantization
│   │   ├── product.rs            # PQ quantization
│   │   └── binary.rs             # Binary quantization
│   ├── operators.rs              # SQL operators (<->, <=>, etc.)
│   └── functions.rs              # SQL functions
└── tests/
    ├── integration_tests.rs
    └── compatibility_tests.rs    # pgvector compatibility
```

## Version History

- **0.1.0**: Initial release with pgvector compatibility
  - HNSW and IVFFlat indexes
  - SIMD-optimized distance functions
  - Scalar quantization support
  - Neon compatibility
  - Zero-copy varlena access
  - AVX-512/AVX2/NEON support

## License

MIT License - Same as ruvector-core
