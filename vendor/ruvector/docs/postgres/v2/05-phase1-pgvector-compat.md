# RuVector Postgres v2 - Phase 1: pgvector Compatibility

## Overview

Phase 1 establishes the foundation: a PostgreSQL extension that is **100% compatible** with pgvector. Any application using pgvector should be able to switch to RuVector by simply changing the extension name while keeping all queries unchanged.

---

## Objectives

### Primary Goals
1. Drop-in replacement for pgvector
2. Same SQL syntax for all operations
3. Equal or better performance on standard benchmarks
4. Pass pgvector's test suite

### Success Criteria
- All pgvector types work identically
- All operators produce same results
- HNSW and IVFFlat indexes work with same syntax
- Recall@10 >= 95% on standard datasets
- Query latency competitive with pgvector

---

## Incremental Implementation Path

**IMPORTANT**: Supporting custom Postgres Access Methods is expensive. We take an incremental path to prove correctness before full AM plumbing.

### Phase 1a: Custom Operator Class + Function Scan (Weeks 1-2)

```
+------------------------------------------------------------------+
|              PHASE 1a: FUNCTION-BASED SEARCH                      |
+------------------------------------------------------------------+

APPROACH:
  • Implement operators (<->, <=>, <#>)
  • Create function that returns TIDs: ruvector_search(query, k)
  • Use function in subquery for ordering

EXAMPLE QUERY:
  SELECT * FROM items
  WHERE ctid = ANY(ruvector_search(embedding, '[1,2,3]', 10))
  ORDER BY embedding <-> '[1,2,3]';

BENEFITS:
  • Validate engine routing and correctness
  • Simpler to debug than full AM
  • Can benchmark before AM investment

LIMITATIONS:
  • Requires query rewrite (not transparent)
  • Two-phase: search then fetch
  • No planner cost estimation

+------------------------------------------------------------------+
```

```rust
/// Phase 1a: Function-based search (no Index AM required)
#[pg_extern]
pub fn ruvector_search(
    collection: &str,
    query: pgrx::composite_type!("vector"),
    k: i32,
) -> SetOfIterator<'static, pgrx::pg_sys::ItemPointerData> {
    let query_vec = extract_vector(query);
    let engine = SharedMemory::get().engine();

    // Submit search to engine
    let results = engine.search(collection, &query_vec, k as usize);

    // Return TIDs
    SetOfIterator::new(results.into_iter().map(|r| r.tid))
}
```

### Phase 1b: Proper Index Access Method (Weeks 3-4)

```
+------------------------------------------------------------------+
|              PHASE 1b: FULL INDEX ACCESS METHOD                   |
+------------------------------------------------------------------+

ONLY AFTER:
  • Phase 1a correctness verified
  • Benchmark results acceptable
  • Recall targets met

IMPLEMENTATION:
  • Register amroutine with all handler functions
  • Implement ambuild, aminsert, amgettuple
  • Add cost estimation for planner
  • Support parallel scan

RESULT:
  • Transparent pgvector-compatible syntax
  • CREATE INDEX ... USING ruhnsw
  • Planner automatically chooses index

+------------------------------------------------------------------+
```

```rust
/// Phase 1b: Full Index AM handler
#[pg_extern]
pub fn ruhnsw_handler(_fcinfo: pg_sys::FunctionCallInfo) -> pg_sys::Datum {
    let mut routine = unsafe { PgBox::<pg_sys::IndexAmRoutine>::alloc0() };

    // AM identification
    routine.amstrategies = 1;
    routine.amsupport = 1;
    routine.amoptionalkey = false;
    routine.amsearcharray = false;
    routine.amsearchnulls = false;
    routine.amstorage = false;
    routine.amclusterable = false;
    routine.ampredlocks = false;
    routine.amcanparallel = true;
    routine.amcaninclude = false;
    routine.amusemaintenanceworkmem = true;
    routine.amparallelvacuumoptions = pg_sys::VACUUM_OPTION_PARALLEL_BULKDEL as u8;

    // Required callbacks
    routine.ambuild = Some(ruhnsw_build);
    routine.ambuildempty = Some(ruhnsw_build_empty);
    routine.aminsert = Some(ruhnsw_insert);
    routine.ambulkdelete = Some(ruhnsw_bulk_delete);
    routine.amvacuumcleanup = Some(ruhnsw_vacuum_cleanup);
    routine.amcostestimate = Some(ruhnsw_cost_estimate);
    routine.amoptions = Some(ruhnsw_options);
    routine.amproperty = None;
    routine.ambuildphasename = None;
    routine.amvalidate = Some(ruhnsw_validate);
    routine.amadjustmembers = None;
    routine.ambeginscan = Some(ruhnsw_begin_scan);
    routine.amrescan = Some(ruhnsw_rescan);
    routine.amgettuple = Some(ruhnsw_gettuple);
    routine.amgetbitmap = None;
    routine.amendscan = Some(ruhnsw_end_scan);
    routine.ammarkpos = None;
    routine.amrestrpos = None;
    routine.amestimateparallelscan = Some(ruhnsw_estimate_parallel);
    routine.aminitparallelscan = Some(ruhnsw_init_parallel);
    routine.amparallelrescan = Some(ruhnsw_parallel_rescan);

    routine.into_pg()
}
```

### Risk Mitigation

```
Phase 1a protects against:
• Weeks spent in AM plumbing before engine correctness proven
• Hard-to-debug issues in complex AM callbacks
• Premature optimization before baseline established

Transition criteria from 1a to 1b:
• [ ] All distance functions match pgvector within epsilon
• [ ] Recall@10 >= 95% on sift-128, gist-960 datasets
• [ ] Query latency within 2x of pgvector on same hardware
• [ ] Insert throughput within 2x of pgvector
```

---

## Deliverables

### 1. Extension Skeleton

```rust
// src/lib.rs
use pgrx::prelude::*;

// Extension magic
::pgrx::pg_module_magic!();

// Version info
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Extension initialization
#[pg_guard]
pub extern "C" fn _PG_init() {
    // Initialize SIMD dispatch
    init_simd_dispatch();

    // Register GUCs
    register_gucs();

    // Register background workers
    register_workers();

    pgrx::log!("RuVector {} initialized", VERSION);
}

// Module structure
pub mod types;      // Vector types
pub mod operators;  // Distance operators
pub mod functions;  // SQL functions
pub mod index;      // Access methods
pub mod distance;   // SIMD distance calculations
```

### 2. Vector Type (pgvector Compatible)

```rust
// src/types/vector.rs
use pgrx::prelude::*;
use std::mem::size_of;

/// PostgreSQL vector type (pgvector compatible)
#[derive(Debug, Clone)]
#[repr(C)]
pub struct RuVector {
    /// Varlena header (required for variable-length types)
    vl_len_: [u8; 4],
    /// Number of dimensions
    dim: u16,
    /// Unused padding
    unused: u16,
    /// Vector data (f32 array)
    data: [f32; 0],  // Flexible array member
}

impl RuVector {
    /// Create new vector from slice
    pub fn from_slice(data: &[f32]) -> *mut RuVector {
        let dim = data.len();
        if dim > 16000 {
            pgrx::error!("vector cannot have more than 16000 dimensions");
        }

        let size = size_of::<RuVector>() + dim * size_of::<f32>();

        unsafe {
            let ptr = pgrx::pg_sys::palloc0(size) as *mut RuVector;

            // Set varlena header
            pgrx::pg_sys::SET_VARSIZE(ptr as *mut _, size as i32);

            // Set dimensions
            (*ptr).dim = dim as u16;
            (*ptr).unused = 0;

            // Copy data
            let data_ptr = (*ptr).data.as_mut_ptr();
            std::ptr::copy_nonoverlapping(data.as_ptr(), data_ptr, dim);

            ptr
        }
    }

    /// Get dimensions
    pub fn dimensions(&self) -> usize {
        self.dim as usize
    }

    /// Get data as slice
    pub fn as_slice(&self) -> &[f32] {
        unsafe {
            std::slice::from_raw_parts(self.data.as_ptr(), self.dim as usize)
        }
    }

    /// Get data as mutable slice
    pub fn as_mut_slice(&mut self) -> &mut [f32] {
        unsafe {
            std::slice::from_raw_parts_mut(self.data.as_mut_ptr(), self.dim as usize)
        }
    }

    /// Calculate L2 norm
    pub fn norm(&self) -> f32 {
        distance::l2_norm(self.as_slice())
    }

    /// Normalize in place
    pub fn normalize(&mut self) {
        let norm = self.norm();
        if norm > 0.0 {
            for x in self.as_mut_slice() {
                *x /= norm;
            }
        }
    }
}

// Input function: '[1.0, 2.0, 3.0]' -> vector
#[pg_extern(immutable, parallel_safe)]
fn vector_in(
    input: &std::ffi::CStr,
    _oid: pg_sys::Oid,
    typmod: i32,
) -> *mut RuVector {
    let s = input.to_str().unwrap_or("");

    // Parse [x, y, z] format
    let trimmed = s.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        pgrx::error!("malformed vector literal: {}", s);
    }

    let inner = &trimmed[1..trimmed.len()-1];
    let values: Vec<f32> = inner
        .split(',')
        .map(|x| x.trim().parse::<f32>())
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|e| pgrx::error!("invalid vector element: {}", e));

    // Check typmod (dimensions)
    if typmod > 0 && values.len() != typmod as usize {
        pgrx::error!(
            "expected {} dimensions, not {}",
            typmod, values.len()
        );
    }

    RuVector::from_slice(&values)
}

// Output function: vector -> '[1.0, 2.0, 3.0]'
#[pg_extern(immutable, parallel_safe)]
fn vector_out(vector: *mut RuVector) -> *mut std::ffi::c_char {
    let vec = unsafe { &*vector };
    let data = vec.as_slice();

    let mut s = String::with_capacity(data.len() * 10);
    s.push('[');
    for (i, val) in data.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("{}", val));
    }
    s.push(']');

    // Allocate in PostgreSQL memory context
    unsafe {
        let len = s.len() + 1;
        let ptr = pgrx::pg_sys::palloc(len) as *mut std::ffi::c_char;
        std::ptr::copy_nonoverlapping(s.as_ptr(), ptr as *mut u8, s.len());
        *ptr.add(s.len()) = 0;
        ptr
    }
}

// Type modifier input: vector(384)
#[pg_extern(immutable, parallel_safe)]
fn vector_typmod_in(modifiers: Vec<&std::ffi::CStr>) -> i32 {
    if modifiers.is_empty() {
        return -1;  // No dimension constraint
    }

    let dim_str = modifiers[0].to_str().unwrap_or("");
    let dim: i32 = dim_str.parse().unwrap_or_else(|_| {
        pgrx::error!("invalid dimension: {}", dim_str)
    });

    if dim < 1 || dim > 16000 {
        pgrx::error!("dimensions must be between 1 and 16000");
    }

    dim
}

// Type modifier output: 384 -> "(384)"
#[pg_extern(immutable, parallel_safe)]
fn vector_typmod_out(typmod: i32) -> *mut std::ffi::c_char {
    if typmod < 0 {
        return std::ptr::null_mut();
    }

    let s = format!("({})", typmod);

    unsafe {
        let ptr = pgrx::pg_sys::palloc(s.len() + 1) as *mut std::ffi::c_char;
        std::ptr::copy_nonoverlapping(s.as_ptr(), ptr as *mut u8, s.len());
        *ptr.add(s.len()) = 0;
        ptr
    }
}
```

### 3. Distance Operators

```rust
// src/operators.rs
use pgrx::prelude::*;

/// L2 (Euclidean) distance: <->
#[pg_extern(immutable, parallel_safe)]
fn vector_l2_distance(
    left: *mut RuVector,
    right: *mut RuVector,
) -> f64 {
    let a = unsafe { (*left).as_slice() };
    let b = unsafe { (*right).as_slice() };

    if a.len() != b.len() {
        pgrx::error!(
            "cannot compute distance between vectors with different dimensions ({} vs {})",
            a.len(), b.len()
        );
    }

    distance::euclidean_distance(a, b) as f64
}

/// Cosine distance: <=>
#[pg_extern(immutable, parallel_safe)]
fn vector_cosine_distance(
    left: *mut RuVector,
    right: *mut RuVector,
) -> f64 {
    let a = unsafe { (*left).as_slice() };
    let b = unsafe { (*right).as_slice() };

    if a.len() != b.len() {
        pgrx::error!(
            "cannot compute distance between vectors with different dimensions ({} vs {})",
            a.len(), b.len()
        );
    }

    distance::cosine_distance(a, b) as f64
}

/// Negative inner product (for MAX inner product via MIN): <#>
#[pg_extern(immutable, parallel_safe)]
fn vector_negative_inner_product(
    left: *mut RuVector,
    right: *mut RuVector,
) -> f64 {
    let a = unsafe { (*left).as_slice() };
    let b = unsafe { (*right).as_slice() };

    if a.len() != b.len() {
        pgrx::error!(
            "cannot compute inner product between vectors with different dimensions ({} vs {})",
            a.len(), b.len()
        );
    }

    -distance::inner_product(a, b) as f64
}

// Convenience functions (for direct SQL calls)

/// cosine_similarity(a, b) = 1 - cosine_distance(a, b)
#[pg_extern(immutable, parallel_safe)]
fn cosine_similarity(
    left: *mut RuVector,
    right: *mut RuVector,
) -> f64 {
    1.0 - vector_cosine_distance(left, right)
}

/// inner_product(a, b) = -negative_inner_product(a, b)
#[pg_extern(immutable, parallel_safe)]
fn inner_product(
    left: *mut RuVector,
    right: *mut RuVector,
) -> f64 {
    -vector_negative_inner_product(left, right)
}

/// l2_distance(a, b) - explicit function
#[pg_extern(immutable, parallel_safe)]
fn l2_distance(
    left: *mut RuVector,
    right: *mut RuVector,
) -> f64 {
    vector_l2_distance(left, right)
}
```

### 4. SIMD Distance Functions

```rust
// src/distance/mod.rs
use std::sync::OnceLock;

/// SIMD implementation selector
pub enum SimdImpl {
    Avx512,
    Avx2,
    Neon,
    Scalar,
}

static SIMD_IMPL: OnceLock<SimdImpl> = OnceLock::new();

/// Initialize SIMD dispatch based on CPU features
pub fn init_simd_dispatch() {
    let impl_choice = detect_simd_impl();
    SIMD_IMPL.set(impl_choice).ok();
}

fn detect_simd_impl() -> SimdImpl {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx512f") {
            return SimdImpl::Avx512;
        }
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            return SimdImpl::Avx2;
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        return SimdImpl::Neon;
    }

    SimdImpl::Scalar
}

/// Get SIMD info string
pub fn simd_info() -> &'static str {
    match SIMD_IMPL.get() {
        Some(SimdImpl::Avx512) => "avx512",
        Some(SimdImpl::Avx2) => "avx2",
        Some(SimdImpl::Neon) => "neon",
        _ => "scalar",
    }
}

/// Euclidean distance (L2)
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    match SIMD_IMPL.get() {
        Some(SimdImpl::Avx512) => unsafe { avx512::euclidean_distance(a, b) },
        Some(SimdImpl::Avx2) => unsafe { avx2::euclidean_distance(a, b) },
        Some(SimdImpl::Neon) => unsafe { neon::euclidean_distance(a, b) },
        _ => scalar::euclidean_distance(a, b),
    }
}

/// Cosine distance (1 - cosine similarity)
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    match SIMD_IMPL.get() {
        Some(SimdImpl::Avx512) => unsafe { avx512::cosine_distance(a, b) },
        Some(SimdImpl::Avx2) => unsafe { avx2::cosine_distance(a, b) },
        Some(SimdImpl::Neon) => unsafe { neon::cosine_distance(a, b) },
        _ => scalar::cosine_distance(a, b),
    }
}

/// Inner product (dot product)
pub fn inner_product(a: &[f32], b: &[f32]) -> f32 {
    match SIMD_IMPL.get() {
        Some(SimdImpl::Avx512) => unsafe { avx512::inner_product(a, b) },
        Some(SimdImpl::Avx2) => unsafe { avx2::inner_product(a, b) },
        Some(SimdImpl::Neon) => unsafe { neon::inner_product(a, b) },
        _ => scalar::inner_product(a, b),
    }
}

/// L2 norm
pub fn l2_norm(a: &[f32]) -> f32 {
    inner_product(a, a).sqrt()
}

// Scalar implementation
mod scalar {
    pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| {
                let diff = x - y;
                diff * diff
            })
            .sum::<f32>()
            .sqrt()
    }

    pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 1.0;  // Maximum distance for zero vectors
        }

        let similarity = dot / (norm_a * norm_b);
        1.0 - similarity.clamp(-1.0, 1.0)
    }

    pub fn inner_product(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }
}

// AVX2 implementation
#[cfg(target_arch = "x86_64")]
mod avx2 {
    use std::arch::x86_64::*;

    #[target_feature(enable = "avx2", enable = "fma")]
    pub unsafe fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        let n = a.len();
        let mut sum = _mm256_setzero_ps();

        let mut i = 0;
        while i + 8 <= n {
            let va = _mm256_loadu_ps(a.as_ptr().add(i));
            let vb = _mm256_loadu_ps(b.as_ptr().add(i));
            let diff = _mm256_sub_ps(va, vb);
            sum = _mm256_fmadd_ps(diff, diff, sum);
            i += 8;
        }

        // Horizontal sum
        let mut result = hsum256_ps(sum);

        // Handle remainder
        while i < n {
            let diff = a[i] - b[i];
            result += diff * diff;
            i += 1;
        }

        result.sqrt()
    }

    #[target_feature(enable = "avx2", enable = "fma")]
    pub unsafe fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
        let n = a.len();
        let mut dot_sum = _mm256_setzero_ps();
        let mut norm_a_sum = _mm256_setzero_ps();
        let mut norm_b_sum = _mm256_setzero_ps();

        let mut i = 0;
        while i + 8 <= n {
            let va = _mm256_loadu_ps(a.as_ptr().add(i));
            let vb = _mm256_loadu_ps(b.as_ptr().add(i));

            dot_sum = _mm256_fmadd_ps(va, vb, dot_sum);
            norm_a_sum = _mm256_fmadd_ps(va, va, norm_a_sum);
            norm_b_sum = _mm256_fmadd_ps(vb, vb, norm_b_sum);

            i += 8;
        }

        let mut dot = hsum256_ps(dot_sum);
        let mut norm_a = hsum256_ps(norm_a_sum);
        let mut norm_b = hsum256_ps(norm_b_sum);

        // Handle remainder
        while i < n {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
            i += 1;
        }

        let norm_a = norm_a.sqrt();
        let norm_b = norm_b.sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 1.0;
        }

        let similarity = dot / (norm_a * norm_b);
        1.0 - similarity.clamp(-1.0, 1.0)
    }

    #[target_feature(enable = "avx2", enable = "fma")]
    pub unsafe fn inner_product(a: &[f32], b: &[f32]) -> f32 {
        let n = a.len();
        let mut sum = _mm256_setzero_ps();

        let mut i = 0;
        while i + 8 <= n {
            let va = _mm256_loadu_ps(a.as_ptr().add(i));
            let vb = _mm256_loadu_ps(b.as_ptr().add(i));
            sum = _mm256_fmadd_ps(va, vb, sum);
            i += 8;
        }

        let mut result = hsum256_ps(sum);

        while i < n {
            result += a[i] * b[i];
            i += 1;
        }

        result
    }

    #[inline]
    #[target_feature(enable = "avx2")]
    unsafe fn hsum256_ps(v: __m256) -> f32 {
        let v128 = _mm_add_ps(
            _mm256_castps256_ps128(v),
            _mm256_extractf128_ps(v, 1),
        );
        let v64 = _mm_add_ps(v128, _mm_movehl_ps(v128, v128));
        let v32 = _mm_add_ss(v64, _mm_shuffle_ps(v64, v64, 0x55));
        _mm_cvtss_f32(v32)
    }
}
```

### 5. Collection Metadata

```sql
-- Extension schema
CREATE SCHEMA IF NOT EXISTS ruvector;

-- Collection registration table
CREATE TABLE ruvector.collections (
    id              SERIAL PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    table_schema    TEXT NOT NULL,
    table_name      TEXT NOT NULL,
    column_name     TEXT NOT NULL,
    dimensions      INTEGER NOT NULL CHECK (dimensions > 0 AND dimensions <= 16000),
    distance_metric TEXT NOT NULL DEFAULT 'l2'
                    CHECK (distance_metric IN ('l2', 'cosine', 'ip')),
    index_type      TEXT CHECK (index_type IN ('hnsw', 'ivfflat', 'flat')),
    index_oid       OID,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    config          JSONB NOT NULL DEFAULT '{}'::jsonb,

    UNIQUE(table_schema, table_name, column_name)
);

-- Index for lookups
CREATE INDEX idx_collections_table
    ON ruvector.collections(table_schema, table_name);

-- Index statistics
CREATE TABLE ruvector.index_stats (
    index_oid       OID PRIMARY KEY,
    collection_id   INTEGER REFERENCES ruvector.collections(id) ON DELETE CASCADE,
    vector_count    BIGINT NOT NULL DEFAULT 0,
    index_size_bytes BIGINT NOT NULL DEFAULT 0,
    last_build      TIMESTAMPTZ,
    last_vacuum     TIMESTAMPTZ,
    build_time_ms   BIGINT,
    avg_search_ms   REAL,
    recall_estimate REAL,
    stats_json      JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### 6. HNSW Index

```rust
// src/index/hnsw.rs
use pgrx::prelude::*;

/// HNSW index structure (in-memory)
pub struct HnswIndex {
    /// Maximum connections per node
    m: usize,
    /// Maximum connections for layer 0
    m0: usize,
    /// Construction ef
    ef_construction: usize,
    /// Entry point node
    entry_point: Option<usize>,
    /// Maximum layer
    max_layer: usize,
    /// Node data
    nodes: Vec<HnswNode>,
    /// Distance function
    distance_fn: DistanceFunction,
}

struct HnswNode {
    /// Vector ID (maps to TID)
    id: TupleId,
    /// Layer this node is on
    layer: usize,
    /// Neighbors per layer
    neighbors: Vec<Vec<usize>>,
}

impl HnswIndex {
    pub fn new(m: usize, ef_construction: usize, distance: DistanceFunction) -> Self {
        Self {
            m,
            m0: m * 2,
            ef_construction,
            entry_point: None,
            max_layer: 0,
            nodes: Vec::new(),
            distance_fn: distance,
        }
    }

    /// Insert vector into index
    pub fn insert(&mut self, id: TupleId, vector: &[f32]) {
        let layer = self.random_layer();
        let node_idx = self.nodes.len();

        // Create node
        let mut node = HnswNode {
            id,
            layer,
            neighbors: vec![Vec::new(); layer + 1],
        };

        if let Some(ep) = self.entry_point {
            // Search for neighbors at each layer
            let mut current_ep = ep;

            // Descend from top layer
            for l in (layer + 1..=self.max_layer).rev() {
                current_ep = self.search_layer(vector, current_ep, 1, l)[0].0;
            }

            // Insert at each layer from layer down to 0
            for l in (0..=layer.min(self.max_layer)).rev() {
                let candidates = self.search_layer(
                    vector,
                    current_ep,
                    self.ef_construction,
                    l
                );

                // Select M neighbors
                let neighbors = self.select_neighbors(
                    vector,
                    &candidates,
                    if l == 0 { self.m0 } else { self.m }
                );

                node.neighbors[l] = neighbors.clone();

                // Add bidirectional connections
                for neighbor_idx in &neighbors {
                    let neighbor = &mut self.nodes[*neighbor_idx];
                    neighbor.neighbors[l].push(node_idx);

                    // Prune if over capacity
                    let max_neighbors = if l == 0 { self.m0 } else { self.m };
                    if neighbor.neighbors[l].len() > max_neighbors {
                        self.prune_neighbors(*neighbor_idx, l, max_neighbors);
                    }
                }

                if !candidates.is_empty() {
                    current_ep = candidates[0].0;
                }
            }
        }

        self.nodes.push(node);

        // Update entry point if new node has higher layer
        if layer > self.max_layer || self.entry_point.is_none() {
            self.entry_point = Some(node_idx);
            self.max_layer = layer;
        }
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize, ef: usize) -> Vec<(TupleId, f32)> {
        let Some(ep) = self.entry_point else {
            return Vec::new();
        };

        let mut current_ep = ep;

        // Descend from top layer to layer 1
        for l in (1..=self.max_layer).rev() {
            current_ep = self.search_layer(query, current_ep, 1, l)[0].0;
        }

        // Search at layer 0 with ef candidates
        let candidates = self.search_layer(query, current_ep, ef.max(k), 0);

        // Return top k
        candidates.into_iter()
            .take(k)
            .map(|(idx, dist)| (self.nodes[idx].id, dist))
            .collect()
    }

    fn search_layer(
        &self,
        query: &[f32],
        entry: usize,
        ef: usize,
        layer: usize,
    ) -> Vec<(usize, f32)> {
        // Greedy search implementation
        let mut visited = std::collections::HashSet::new();
        let mut candidates = std::collections::BinaryHeap::new();
        let mut results = std::collections::BinaryHeap::new();

        let entry_dist = self.distance(query, entry);
        visited.insert(entry);
        candidates.push(std::cmp::Reverse((OrderedFloat(entry_dist), entry)));
        results.push((OrderedFloat(entry_dist), entry));

        while let Some(std::cmp::Reverse((OrderedFloat(c_dist), c_idx))) = candidates.pop() {
            let furthest_dist = results.peek().map(|(d, _)| d.0).unwrap_or(f32::MAX);

            if c_dist > furthest_dist && results.len() >= ef {
                break;
            }

            for &neighbor in &self.nodes[c_idx].neighbors[layer] {
                if visited.insert(neighbor) {
                    let dist = self.distance(query, neighbor);

                    if dist < furthest_dist || results.len() < ef {
                        candidates.push(std::cmp::Reverse((OrderedFloat(dist), neighbor)));
                        results.push((OrderedFloat(dist), neighbor));

                        if results.len() > ef {
                            results.pop();
                        }
                    }
                }
            }
        }

        let mut result_vec: Vec<_> = results.into_iter()
            .map(|(d, idx)| (idx, d.0))
            .collect();
        result_vec.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        result_vec
    }

    fn random_layer(&self) -> usize {
        let ml = 1.0 / (self.m as f32).ln();
        let mut rng = rand::thread_rng();
        let r: f32 = rng.gen();
        (-r.ln() * ml).floor() as usize
    }

    fn select_neighbors(
        &self,
        query: &[f32],
        candidates: &[(usize, f32)],
        m: usize,
    ) -> Vec<usize> {
        // Simple selection: take closest M
        candidates.iter()
            .take(m)
            .map(|(idx, _)| *idx)
            .collect()
    }

    fn prune_neighbors(&mut self, node_idx: usize, layer: usize, max_neighbors: usize) {
        // Keep closest neighbors
        let node = &mut self.nodes[node_idx];
        if node.neighbors[layer].len() <= max_neighbors {
            return;
        }

        // Sort by distance and keep top M
        // (Implementation simplified - full version would recompute distances)
        node.neighbors[layer].truncate(max_neighbors);
    }

    fn distance(&self, query: &[f32], node_idx: usize) -> f32 {
        // Get vector from storage and compute distance
        // Simplified: in full implementation, this accesses the vector storage
        0.0  // Placeholder
    }
}
```

### 7. SQL Extension Setup

```sql
-- extension/ruvector--0.1.0.sql

-- Type creation
CREATE TYPE vector;

CREATE FUNCTION vector_in(cstring, oid, integer) RETURNS vector
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_out(vector) RETURNS cstring
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_typmod_in(cstring[]) RETURNS integer
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_typmod_out(integer) RETURNS cstring
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_recv(internal, oid, integer) RETURNS vector
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_send(vector) RETURNS bytea
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE TYPE vector (
    INPUT     = vector_in,
    OUTPUT    = vector_out,
    TYPMOD_IN = vector_typmod_in,
    TYPMOD_OUT = vector_typmod_out,
    RECEIVE   = vector_recv,
    SEND      = vector_send,
    STORAGE   = external,
    INTERNALLENGTH = VARIABLE,
    ALIGNMENT = double
);

-- Distance functions
CREATE FUNCTION vector_l2_distance(vector, vector) RETURNS float8
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_cosine_distance(vector, vector) RETURNS float8
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_negative_inner_product(vector, vector) RETURNS float8
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

-- Operators
CREATE OPERATOR <-> (
    LEFTARG   = vector,
    RIGHTARG  = vector,
    FUNCTION  = vector_l2_distance,
    COMMUTATOR = <->
);

CREATE OPERATOR <=> (
    LEFTARG   = vector,
    RIGHTARG  = vector,
    FUNCTION  = vector_cosine_distance,
    COMMUTATOR = <=>
);

CREATE OPERATOR <#> (
    LEFTARG   = vector,
    RIGHTARG  = vector,
    FUNCTION  = vector_negative_inner_product,
    COMMUTATOR = <#>
);

-- Access methods
CREATE ACCESS METHOD hnsw TYPE INDEX HANDLER hnsw_handler;
CREATE ACCESS METHOD ivfflat TYPE INDEX HANDLER ivfflat_handler;

-- Operator classes
CREATE OPERATOR CLASS vector_l2_ops
    DEFAULT FOR TYPE vector USING hnsw AS
    OPERATOR 1 <-> (vector, vector) FOR ORDER BY float_ops,
    FUNCTION 1 vector_l2_distance(vector, vector);

CREATE OPERATOR CLASS vector_cosine_ops
    FOR TYPE vector USING hnsw AS
    OPERATOR 1 <=> (vector, vector) FOR ORDER BY float_ops,
    FUNCTION 1 vector_cosine_distance(vector, vector);

CREATE OPERATOR CLASS vector_ip_ops
    FOR TYPE vector USING hnsw AS
    OPERATOR 1 <#> (vector, vector) FOR ORDER BY float_ops,
    FUNCTION 1 vector_negative_inner_product(vector, vector);

-- Utility functions
CREATE FUNCTION vector_dims(vector) RETURNS integer
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_norm(vector) RETURNS real
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION l2_distance(vector, vector) RETURNS float8
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION cosine_similarity(vector, vector) RETURNS float8
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION inner_product(vector, vector) RETURNS float8
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;
```

---

## Testing Plan

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_creation() {
        let data = vec![1.0, 2.0, 3.0];
        let vec = RuVector::from_slice(&data);
        assert_eq!(unsafe { (*vec).dimensions() }, 3);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![3.0, 4.0, 0.0];
        assert!((euclidean_distance(&a, &b) - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_distance() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!((cosine_distance(&a, &b) - 1.0).abs() < 0.001);  // Perpendicular
    }
}
```

### Integration Tests

```sql
-- test/sql/basic.sql

-- Create extension
CREATE EXTENSION ruvector;

-- Create table with vector column
CREATE TABLE items (
    id SERIAL PRIMARY KEY,
    embedding vector(3)
);

-- Insert vectors
INSERT INTO items (embedding) VALUES
    ('[1, 2, 3]'),
    ('[4, 5, 6]'),
    ('[1, 1, 1]');

-- Test operators
SELECT id, embedding <-> '[1, 2, 3]' AS distance
FROM items
ORDER BY embedding <-> '[1, 2, 3]'
LIMIT 2;

-- Create HNSW index
CREATE INDEX ON items USING hnsw (embedding vector_l2_ops);

-- Test indexed search
SELECT id, embedding <-> '[1, 2, 3]' AS distance
FROM items
ORDER BY embedding <-> '[1, 2, 3]'
LIMIT 2;

-- Verify same results
```

### Recall Benchmark

```python
# benchmark/recall_test.py
import numpy as np
import psycopg2

def test_recall(conn, n_vectors=100000, dims=128, k=10):
    """Test recall@k against brute force"""

    # Generate random vectors
    vectors = np.random.randn(n_vectors, dims).astype(np.float32)
    query = np.random.randn(dims).astype(np.float32)

    # Insert vectors
    cur = conn.cursor()
    for i, vec in enumerate(vectors):
        cur.execute(
            "INSERT INTO bench (embedding) VALUES (%s)",
            (vec.tolist(),)
        )

    # Create index
    cur.execute("""
        CREATE INDEX ON bench
        USING hnsw (embedding vector_l2_ops)
        WITH (m = 16, ef_construction = 64)
    """)

    # Set ef_search
    cur.execute("SET ruvector.ef_search = 100")

    # Get HNSW results
    cur.execute("""
        SELECT id FROM bench
        ORDER BY embedding <-> %s
        LIMIT %s
    """, (query.tolist(), k))
    hnsw_results = set(row[0] for row in cur.fetchall())

    # Get brute force results
    cur.execute("DROP INDEX bench_embedding_idx")
    cur.execute("""
        SELECT id FROM bench
        ORDER BY embedding <-> %s
        LIMIT %s
    """, (query.tolist(), k))
    exact_results = set(row[0] for row in cur.fetchall())

    # Compute recall
    recall = len(hnsw_results & exact_results) / k
    print(f"Recall@{k}: {recall:.2%}")

    return recall >= 0.95  # Success if recall >= 95%
```

---

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| pgrx | 0.11+ | PostgreSQL extension framework |
| rand | 0.8+ | Random number generation |
| ordered-float | 3.0+ | Float ordering for heaps |
| parking_lot | 0.12+ | Synchronization |

---

## Timeline

| Week | Deliverable |
|------|-------------|
| 1 | Extension skeleton, vector type |
| 2 | Operators, distance functions |
| 3 | HNSW index AM |
| 4 | IVFFlat index AM, testing |

---

## Blockers

- PostgreSQL development headers required
- pgrx version compatibility with target PG versions
- SIMD availability for performance testing
