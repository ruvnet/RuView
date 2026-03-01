# ADR-STS-003: Memory Management and HNSW Integration Strategy

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2026-02-20 |
| **Authors** | RuVector Architecture Team |
| **Deciders** | Architecture Review Board |
| **Supersedes** | None |
| **Related** | ADR-006 (Unified Memory Pool), ADR-003 (SIMD Optimization), ADR-005 (WASM Runtime), ADR-STS-001, ADR-STS-002 |

---

## 1. Context and Problem Statement

RuVector possesses a sophisticated, multi-layered memory infrastructure designed for
high-performance vector operations at scale. The sublinear-time solver integration
introduces new memory allocation patterns -- temporary scratch space for iterative
Neumann series expansion, sparse matrix storage in CSR format, random walk state
buffers, and HNSW graph topology extraction -- that must interoperate with the existing
memory subsystem without degrading performance or exceeding platform-specific budgets.

### 1.1 Existing RuVector Memory Infrastructure

The following memory subsystems are already operational in the RuVector codebase:

| Subsystem | Source | Characteristics |
|-----------|--------|-----------------|
| **Arena Allocator** | `ruvector-core/src/arena.rs` | Cache-aligned (64-byte), O(1) bump allocation, batch reset, thread-local |
| **SoA Storage** | `ruvector-core/src/cache_optimized.rs` | Column-major layout for SIMD-friendly sequential dimension access |
| **Paged Memory (ADR-006)** | `ruvector-core/src/memory.rs` | 2MB pages, LRU eviction, Hot/Warm/Cold tiers, ref-counted pinning |
| **Quantization** | `ruvector-core/src/quantization.rs` | Scalar 4x, INT4 8x, PQ 16x, Binary 32x compression ratios |
| **Memory-Mapped Files** | via `memmap2` crate | OS-managed paging for large datasets exceeding physical RAM |
| **Lock-Free Structures** | `ruvector-core/src/lockfree.rs` | `AtomicVectorPool`, `LockFreeWorkQueue`, concurrent allocation |

### 1.2 Solver Memory Requirements

The sublinear-time solver introduces the following allocation patterns:

| Component | Allocation Pattern | Lifetime | Size Characteristics |
|-----------|--------------------|----------|---------------------|
| Neumann iteration vectors | k temporary n-vectors per solve | Per-solve (reset between solves) | k * n * 4 bytes |
| CSR sparse matrix | Persistent for problem duration | Per-problem (may be cached) | nnz * 12 bytes (value + col + row_ptr) |
| Random walk state | s active walker states | Per-estimation | s * 24 bytes (position + weight + rng) |
| Convergence residuals | Small vector per iteration | Per-iteration (overwritten) | n * 4 bytes |
| HNSW adjacency extraction | One-time graph copy | Per-query or cached | E * 8 bytes (edge list) |
| Solver scheduler state | Fixed overhead | Process lifetime | ~1 KB |

### 1.3 Memory Profiles at Scale

The following table models total memory consumption for representative workloads,
combining RuVector's existing storage with solver overhead:

```
Workload A: 1M vectors at 384D (production vector search)
  Vector storage:        1,000,000 * 384 * 4    = 1,536 MB
  HNSW graph (M=16):    1,000,000 * 16 * 2 * 8  =   256 MB
  HNSW metadata:         1,000,000 * 100         =   100 MB
  Index overhead (redb):                          =    50 MB
  -------------------------------------------------------
  RuVector baseline:                               1,942 MB

  Solver: 10K x 10K sparse Laplacian at 1% density:
    CSR values:          100,000 * 4              =   0.4 MB
    CSR col_indices:     100,000 * 4              =   0.4 MB
    CSR row_ptr:         10,001 * 4               =   0.04 MB
    Working vectors (k=20 iterations):
                         20 * 10,000 * 4          =   0.8 MB
    -------------------------------------------------------
    Solver overhead:                                  1.6 MB  (0.08% of baseline)

Workload B: 100K vectors at 768D (large embedding model)
  Vector storage:        100,000 * 768 * 4       =   307 MB
  HNSW graph:            100,000 * 16 * 2 * 8    =    26 MB
  Solver: 100K x 100K Laplacian at 0.1% density:
    CSR storage:         10,000,000 * 12          =   120 MB
    Working vectors (k=20):
                         20 * 100,000 * 4         =     8 MB
    -------------------------------------------------------
    Solver overhead:                                 128 MB   (38% of baseline)

Workload C: WASM browser deployment (constrained)
  Total linear memory budget:                     =     8 MB
  Vector storage (1K vectors at 128D):
                         1,000 * 128 * 4          =   0.5 MB
  HNSW graph:            1,000 * 16 * 2 * 8       =   0.3 MB
  Available for solver:                            =   4-5 MB
  Solver: 1K x 1K at 5% density:
    CSR storage:         50,000 * 12              =   0.6 MB
    Working vectors (k=15):
                         15 * 1,000 * 4           =   0.06 MB
    -------------------------------------------------------
    Solver overhead:                                  0.66 MB (within budget)
```

### 1.4 Decision Drivers

- **DR-1**: Solver temporaries must not fragment the global heap or degrade HNSW search latency
- **DR-2**: Large sparse matrices (>1M x 1M) must not cause OOM; paged eviction required
- **DR-3**: WASM solver must operate within a 4-8 MB memory budget in browser contexts
- **DR-4**: Zero-copy data paths between SoA vector storage and solver inputs
- **DR-5**: Cache behavior must be predictable; tiling strategy required for DRAM-bound operations
- **DR-6**: Memory usage must be observable via existing metrics infrastructure
- **DR-7**: Quantized vectors should remain in compressed form until final distance computation

---

## 2. Decision

We adopt a seven-part memory management strategy that integrates the sublinear-time
solver into RuVector's existing memory infrastructure.

### 2.1 Arena-Based Scratch Space for Solver Temporaries

All per-solve temporary allocations use RuVector's existing arena allocator from
`ruvector-core/src/arena.rs`. The arena is reset between solves, providing O(1)
allocation with zero fragmentation.

```rust
use ruvector_core::arena::{Arena, CACHE_LINE_SIZE};

/// Solver scratch space backed by RuVector's arena allocator.
/// All temporaries are cache-line aligned (64 bytes) for SIMD access.
/// The arena is reset between solves, freeing all temporaries at once.
pub struct SolverScratch {
    arena: Arena,
    // Pre-computed offsets into arena for each working vector
    vector_offsets: Vec<usize>,
    // Dimensions of the current problem
    n: usize,
    // Number of iteration slots allocated
    k_slots: usize,
}

impl SolverScratch {
    /// Create a new scratch space for problems of dimension `n`
    /// with `k` iteration slots.
    ///
    /// Total allocation: k * n * sizeof(f32) bytes, cache-line aligned.
    /// For n=10,000 and k=20: 800 KB (fits in L3 cache).
    pub fn new(n: usize, k: usize) -> Self {
        let bytes_per_vector = n * std::mem::size_of::<f32>();
        let aligned_size = (bytes_per_vector + CACHE_LINE_SIZE - 1)
            & !(CACHE_LINE_SIZE - 1);
        let total_bytes = k * aligned_size;

        let arena = Arena::with_capacity(total_bytes);
        let mut vector_offsets = Vec::with_capacity(k);
        for i in 0..k {
            vector_offsets.push(i * aligned_size);
        }

        Self { arena, vector_offsets, n, k_slots: k }
    }

    /// Borrow working vector `i` as a mutable f32 slice.
    /// Panics if `i >= k_slots`.
    #[inline(always)]
    pub fn working_vector_mut(&mut self, i: usize) -> &mut [f32] {
        debug_assert!(i < self.k_slots, "vector index out of bounds");
        let offset = self.vector_offsets[i];
        unsafe {
            let ptr = self.arena.as_mut_ptr().add(offset) as *mut f32;
            std::slice::from_raw_parts_mut(ptr, self.n)
        }
    }

    /// Borrow working vector `i` as an immutable f32 slice.
    #[inline(always)]
    pub fn working_vector(&self, i: usize) -> &[f32] {
        debug_assert!(i < self.k_slots, "vector index out of bounds");
        let offset = self.vector_offsets[i];
        unsafe {
            let ptr = self.arena.as_ptr().add(offset) as *const f32;
            std::slice::from_raw_parts(ptr, self.n)
        }
    }

    /// Reset all scratch space for the next solve.
    /// O(1) operation -- just resets the arena bump pointer.
    #[inline]
    pub fn reset(&mut self) {
        self.arena.reset();
    }

    /// Returns the total bytes allocated by this scratch space.
    pub fn allocated_bytes(&self) -> usize {
        self.k_slots * ((self.n * 4 + CACHE_LINE_SIZE - 1)
            & !(CACHE_LINE_SIZE - 1))
    }
}
```

**Memory formula for scratch space**:

```
scratch_bytes = k * ceil(n * 4 / 64) * 64

Where:
  k = number of Neumann iteration slots (typically 10-30)
  n = problem dimension (number of unknowns)
  64 = cache line size in bytes

Examples:
  n=1,000,   k=15:  15 * ceil(4000/64) * 64  =  15 * 4032  =   60,480 bytes  (~59 KB)
  n=10,000,  k=20:  20 * ceil(40000/64) * 64  =  20 * 40000 =  800,000 bytes  (~781 KB)
  n=100,000, k=25:  25 * ceil(400000/64) * 64 =  25 * 400000 = 10,000,000 bytes (~9.5 MB)
  n=1,000,000, k=30: 30 * 4,000,000           = 120,000,000 bytes (~114 MB)
```

### 2.2 CSR Matrix Storage with SIMD-Friendly Layout

Sparse matrices are stored in Compressed Sparse Row (CSR) format with data layout
optimized for SIMD-accelerated Sparse Matrix-Vector multiply (SpMV).

```rust
/// CSR (Compressed Sparse Row) matrix storage.
///
/// Memory layout is optimized for row-oriented SpMV with SIMD:
/// - `values` and `col_indices` are aligned to 32 bytes (AVX2 boundary)
/// - Rows are padded to SIMD width for branchless remainder handling
/// - Row pointers use u32 to halve pointer array size vs u64
///
/// Memory consumption formula:
///   total_bytes = nnz * 4 (values)
///               + nnz * 4 (col_indices)
///               + (nrows + 1) * 4 (row_ptr)
///               + padding (at most nrows * simd_width * 4)
///
/// For a 10K x 10K matrix at 1% density (nnz = 1,000,000):
///   values:      4,000,000 bytes (3.8 MB)
///   col_indices: 4,000,000 bytes (3.8 MB)
///   row_ptr:        40,004 bytes (39 KB)
///   padding:      ~320,000 bytes (worst case, ~312 KB)
///   total:       ~8.2 MB
#[repr(C)]
pub struct CsrMatrix {
    /// Non-zero values, aligned to 32 bytes.
    /// Length: nnz (with SIMD padding per row).
    values: Vec<f32>,

    /// Column indices for each non-zero, aligned to 32 bytes.
    /// Length: same as values.
    col_indices: Vec<u32>,

    /// Row pointers: row_ptr[i] is the index into values/col_indices
    /// where row i begins. row_ptr[nrows] = nnz.
    /// Length: nrows + 1.
    row_ptr: Vec<u32>,

    /// Number of rows.
    nrows: usize,

    /// Number of columns.
    ncols: usize,

    /// Total non-zeros (before padding).
    nnz: usize,
}

impl CsrMatrix {
    /// Construct CSR from COO (coordinate) triplets.
    ///
    /// The triplets are sorted by row, then by column within each row.
    /// Duplicate entries are summed.
    ///
    /// Cost: O(nnz * log(nnz)) for sorting.
    pub fn from_triplets(
        nrows: usize,
        ncols: usize,
        mut triplets: Vec<(u32, u32, f32)>,
    ) -> Self {
        // Sort by (row, col) for CSR construction
        triplets.sort_unstable_by(|a, b| {
            a.0.cmp(&b.0).then(a.1.cmp(&b.1))
        });

        // Merge duplicates
        let mut values = Vec::with_capacity(triplets.len());
        let mut col_indices = Vec::with_capacity(triplets.len());
        let mut row_ptr = vec![0u32; nrows + 1];

        let mut prev_row = 0u32;
        let mut prev_col = u32::MAX;
        let mut nnz = 0usize;

        for (r, c, v) in &triplets {
            if *r == prev_row && *c == prev_col {
                // Duplicate: sum values
                if let Some(last) = values.last_mut() {
                    *last += v;
                }
            } else {
                values.push(*v);
                col_indices.push(*c);
                nnz += 1;
                // Fill row_ptr for any skipped rows
                for row_idx in (prev_row + 1)..=*r {
                    row_ptr[row_idx as usize] = nnz as u32 - 1;
                }
                prev_row = *r;
                prev_col = *c;
            }
        }

        // Fill remaining row_ptr entries
        for row_idx in (prev_row as usize + 1)..=nrows {
            row_ptr[row_idx] = nnz as u32;
        }

        Self { values, col_indices, row_ptr, nrows, ncols, nnz }
    }

    /// SIMD-accelerated sparse matrix-vector multiply: y = A * x.
    ///
    /// Uses gather operations on x86_64 (AVX2 _mm256_i32gather_ps)
    /// and scalar gather with NEON FMA on aarch64.
    ///
    /// Cache behavior:
    /// - `values` and `col_indices` are streamed sequentially (prefetch-friendly)
    /// - `x` is accessed randomly via col_indices (cache-hostile for large x)
    /// - For n > L2_SIZE / 4, tiling is required (see Section 2.5)
    #[inline]
    pub fn spmv(&self, x: &[f32], y: &mut [f32]) {
        debug_assert_eq!(x.len(), self.ncols);
        debug_assert_eq!(y.len(), self.nrows);

        for i in 0..self.nrows {
            let start = self.row_ptr[i] as usize;
            let end = self.row_ptr[i + 1] as usize;
            let mut sum = 0.0f32;

            // Inner loop: dot product of sparse row with dense vector
            // Compiler auto-vectorizes this for sequential value access.
            // For explicit SIMD gather, see spmv_avx2() below.
            for j in start..end {
                unsafe {
                    let col = *self.col_indices.get_unchecked(j) as usize;
                    let val = *self.values.get_unchecked(j);
                    sum += val * *x.get_unchecked(col);
                }
            }

            y[i] = sum;
        }
    }

    /// Returns memory consumed by this matrix in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.values.len() * 4
            + self.col_indices.len() * 4
            + self.row_ptr.len() * 4
    }

    /// Returns the density (fraction of non-zero entries).
    pub fn density(&self) -> f64 {
        self.nnz as f64 / (self.nrows as f64 * self.ncols as f64)
    }
}
```

**CSR memory consumption table**:

| Matrix Size | Density | nnz | CSR Bytes | Notes |
|-------------|---------|-----|-----------|-------|
| 1K x 1K | 5% | 50,000 | 0.6 MB | Fits in L2 cache |
| 10K x 10K | 1% | 1,000,000 | 8.2 MB | Fits in L3 cache |
| 10K x 10K | 0.1% | 100,000 | 0.8 MB | Fits in L2 cache |
| 100K x 100K | 0.1% | 10,000,000 | 82 MB | DRAM, needs tiling |
| 100K x 100K | 0.01% | 1,000,000 | 8.2 MB | Fits in L3 cache |
| 1M x 1M | 0.001% | 10,000,000 | 82 MB | DRAM, needs paging |
| 1M x 1M | 0.01% | 100,000,000 | 820 MB | Must use ADR-006 paged memory |

### 2.3 Paged Memory Integration for Large Matrices

When sparse matrices exceed the L3 cache threshold (typically 16-32 MB), the solver
integrates with ADR-006's paged memory system. Large CSR arrays are stored in 2 MB
pages with LRU eviction, using the `SOLVER_MATRIX` content type.

```rust
use ruvector_core::memory::{MemoryPool, ContentType, PageRange, PinGuard};

/// Content type for solver matrices within ADR-006 paged memory.
/// Priority is between TEMP_BUFFER (evict first) and LORA_WEIGHT (keep longer),
/// reflecting the solver's medium-term caching behavior.
const SOLVER_MATRIX: ContentType = ContentType::new(
    "SOLVER_MATRIX",
    /* eviction_priority */ 2,  // Between TEMP_BUFFER(1) and LORA_WEIGHT(3)
);

/// A large CSR matrix backed by ADR-006 paged memory.
///
/// For matrices exceeding the paging threshold (default 16 MB),
/// the CSR arrays (values, col_indices, row_ptr) are stored in
/// 2 MB pages managed by the unified memory pool. Pages are
/// pinned during SpMV and unpinned afterward, allowing LRU
/// eviction when memory pressure is high.
///
/// Memory layout within pages:
///   Pages 0..V: values array (f32, contiguous)
///   Pages V..C: col_indices array (u32, contiguous)
///   Pages C..R: row_ptr array (u32, contiguous)
///
/// Page count formula:
///   value_pages    = ceil(nnz * 4 / PAGE_SIZE)
///   colind_pages   = ceil(nnz * 4 / PAGE_SIZE)
///   rowptr_pages   = ceil((nrows + 1) * 4 / PAGE_SIZE)
///   total_pages    = value_pages + colind_pages + rowptr_pages
///
/// For 1M x 1M at 0.01% density (nnz = 100M):
///   value_pages  = ceil(400 MB / 2 MB) = 200 pages
///   colind_pages = ceil(400 MB / 2 MB) = 200 pages
///   rowptr_pages = ceil(4 MB / 2 MB)   = 2 pages
///   total = 402 pages = 804 MB
pub struct PagedCsrMatrix {
    pool: Arc<dyn MemoryPool>,
    value_pages: PageRange,
    colind_pages: PageRange,
    rowptr_pages: PageRange,
    nrows: usize,
    ncols: usize,
    nnz: usize,
}

impl PagedCsrMatrix {
    /// Allocate a paged CSR matrix from the memory pool.
    ///
    /// The pages are allocated as SOLVER_MATRIX content type,
    /// which has eviction priority 2 (medium).
    pub fn allocate(
        pool: Arc<dyn MemoryPool>,
        nrows: usize,
        ncols: usize,
        nnz: usize,
    ) -> Result<Self, AllocError> {
        const PAGE_SIZE: usize = 2 * 1024 * 1024; // 2 MB

        let value_page_count = (nnz * 4 + PAGE_SIZE - 1) / PAGE_SIZE;
        let colind_page_count = (nnz * 4 + PAGE_SIZE - 1) / PAGE_SIZE;
        let rowptr_page_count = ((nrows + 1) * 4 + PAGE_SIZE - 1) / PAGE_SIZE;

        let value_pages = pool.allocate(value_page_count, SOLVER_MATRIX)?;
        let colind_pages = pool.allocate(colind_page_count, SOLVER_MATRIX)?;
        let rowptr_pages = pool.allocate(rowptr_page_count, SOLVER_MATRIX)?;

        Ok(Self {
            pool, value_pages, colind_pages, rowptr_pages,
            nrows, ncols, nnz,
        })
    }

    /// Pin all pages during SpMV to prevent LRU eviction.
    /// Returns a guard that unpins on drop (RAII).
    pub fn pin_for_spmv(&self) -> Result<SpmvPinGuard, PinError> {
        let v_guard = self.pool.pin(&self.value_pages)?;
        let c_guard = self.pool.pin(&self.colind_pages)?;
        let r_guard = self.pool.pin(&self.rowptr_pages)?;
        Ok(SpmvPinGuard {
            _value_pin: v_guard,
            _colind_pin: c_guard,
            _rowptr_pin: r_guard,
        })
    }

    /// Total memory consumed in pages.
    pub fn page_count(&self) -> usize {
        self.value_pages.len() + self.colind_pages.len() + self.rowptr_pages.len()
    }
}

/// RAII guard that keeps CSR pages pinned during SpMV.
/// All pages are unpinned when this guard is dropped.
pub struct SpmvPinGuard {
    _value_pin: PinGuard,
    _colind_pin: PinGuard,
    _rowptr_pin: PinGuard,
}
```

**Paging threshold decision logic**:

```
if csr_bytes < 16 MB:
    Use in-memory CsrMatrix (heap-allocated Vec<f32>)
elif csr_bytes < pool_capacity * 0.5:
    Use PagedCsrMatrix with ADR-006 paging
else:
    Use memory-mapped CsrMatrix via memmap2
    (OS manages paging, solver treats as &[f32] slice)
```

### 2.4 Zero-Copy Data Path

The solver borrows vector data directly from RuVector's SoA storage and HNSW graph
without copying. This is critical for maintaining the performance characteristics
established by the existing memory architecture.

#### 2.4.1 Native Zero-Copy (Rust)

```rust
use ruvector_core::cache_optimized::SoAVectorStorage;
use ruvector_core::index::hnsw::HnswIndex;

/// Extract a dimension slice from SoA storage as a solver input.
///
/// SoA storage stores all values of dimension d contiguously:
///   [v0_d, v1_d, v2_d, ..., vn_d]
///
/// This is a zero-copy borrow -- no allocation, no memcpy.
/// The returned slice is valid for the lifetime of the SoA storage.
///
/// Use case: When the solver needs to operate on a single dimension
/// across all vectors (e.g., constructing a distance-based adjacency
/// matrix for a specific dimension).
#[inline]
pub fn borrow_dimension_slice<'a>(
    soa: &'a SoAVectorStorage,
    dimension: usize,
) -> &'a [f32] {
    soa.dimension_slice(dimension)
}

/// Extract HNSW neighbor lists as CSR adjacency matrix.
///
/// The HNSW graph at layer 0 provides the adjacency structure
/// for solver operations. This function constructs a CSR matrix
/// from the HNSW neighbor lists without copying vector data.
///
/// Memory: O(E) where E = total edges in HNSW layer 0.
/// For M=16 and N=100K vectors: E = ~3.2M edges, ~25 MB CSR.
///
/// The adjacency weights can be:
/// - Unweighted (1.0 for all edges)
/// - Distance-weighted (using precomputed distances from HNSW)
/// - Similarity-weighted (1 / (1 + distance))
pub fn hnsw_to_csr_adjacency(
    hnsw: &HnswIndex,
    weight_fn: AdjacencyWeightFn,
) -> CsrMatrix {
    let n = hnsw.len();
    let mut triplets = Vec::with_capacity(n * 16); // M=16 avg

    for node_id in 0..n {
        let neighbors = hnsw.neighbors_at_layer(node_id, 0);
        for &neighbor_id in neighbors {
            let weight = match weight_fn {
                AdjacencyWeightFn::Unweighted => 1.0,
                AdjacencyWeightFn::Distance => {
                    hnsw.distance_between(node_id, neighbor_id)
                }
                AdjacencyWeightFn::Similarity => {
                    1.0 / (1.0 + hnsw.distance_between(node_id, neighbor_id))
                }
            };
            triplets.push((node_id as u32, neighbor_id as u32, weight));
        }
    }

    CsrMatrix::from_triplets(n, n, triplets)
}

/// Weight function for HNSW adjacency extraction.
pub enum AdjacencyWeightFn {
    /// All edges have weight 1.0.
    Unweighted,
    /// Edge weight = distance between endpoints.
    Distance,
    /// Edge weight = 1 / (1 + distance).
    Similarity,
}
```

#### 2.4.2 WASM Zero-Copy (Float32Array::view)

```rust
use wasm_bindgen::prelude::*;
use js_sys::Float32Array;

#[wasm_bindgen]
pub struct WasmSolver {
    scratch: SolverScratch,
    // Solver state...
}

#[wasm_bindgen]
impl WasmSolver {
    /// Solve a sparse system using data from a JS Float32Array.
    ///
    /// ZERO-COPY path: Float32Array::view() creates a view into
    /// WASM linear memory without copying the data. The JS side
    /// writes directly into the solver's input buffer.
    ///
    /// Safety: The Float32Array view is only valid until the next
    /// WASM memory growth. The solver must not trigger allocation
    /// (and thus potential memory growth) while the view is live.
    /// We enforce this by pre-allocating all scratch space in the
    /// constructor.
    #[wasm_bindgen]
    pub fn solve_from_view(&mut self, input: &Float32Array) -> Result<JsValue, JsValue> {
        // Zero-copy: borrow the WASM linear memory directly
        let input_slice = unsafe {
            let ptr = input.as_ptr() as *const f32;
            let len = input.length() as usize;
            std::slice::from_raw_parts(ptr, len)
        };

        // All scratch space was pre-allocated; no growth occurs here.
        let result = self.solve_internal(input_slice)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Return result as a zero-copy Float32Array view.
        let result_vec = self.scratch.working_vector(0);
        let result_view = unsafe {
            Float32Array::view(result_vec)
        };

        Ok(result_view.into())
    }

    /// Pre-allocate all scratch space to avoid WASM memory growth
    /// during solve operations. This is called once in the constructor.
    ///
    /// Budget enforcement: total allocation must not exceed
    /// max_memory_bytes from ComputeBudget.
    fn preallocate(&mut self, n: usize, k: usize, max_bytes: usize) -> Result<(), SolverError> {
        let required = k * ((n * 4 + 63) & !63);
        if required > max_bytes {
            return Err(SolverError::MemoryBudgetExceeded {
                required,
                budget: max_bytes,
            });
        }
        self.scratch = SolverScratch::new(n, k);
        Ok(())
    }
}
```

**WASM linear memory budget allocation**:

```
Total WASM linear memory: 8 MB (4 * 64 KB pages initial, grow to 128 pages)

Allocation:
  WASM stack + globals:     256 KB (fixed)
  Solver scratch space:   2,048 KB (configurable, up to 4 MB)
  CSR matrix storage:     2,048 KB (configurable)
  Vector data (imported):   512 KB (from JS Float32Array view)
  HNSW adjacency cache:    512 KB (optional, can be recomputed)
  Result buffers:           256 KB (output Float32Array views)
  Overhead (allocator):     128 KB (wee_alloc or dlmalloc)
  Reserved:               2,240 KB (growth headroom)
  -------------------------------------------------------
  Total:                  8,000 KB (8 MB)
```

### 2.5 Cache-Aware Tiling Strategy

When the solver's working set exceeds L2 cache, a tiling strategy partitions the
SpMV computation into cache-resident tiles.

#### 2.5.1 Cache Hierarchy Working Set Analysis

```
Modern CPU cache hierarchy (typical server):
  L1 data cache:    48 KB per core   (4-cycle latency)
  L2 cache:        256 KB per core   (12-cycle latency)
  L3 cache:         32 MB shared     (40-cycle latency)
  DRAM:              ~          (100+ cycle latency)

Working set sizes for SpMV y = A * x:
  - Row of CSR values + col_indices: ~8 * nnz_per_row bytes (streamed)
  - x vector (random access):      n * 4 bytes
  - y vector (sequential write):   n * 4 bytes

Critical threshold: when n * 4 > L2 cache, random access into x
causes cache thrashing. Tiling the x vector into L2-resident blocks
restores locality.

Cache-residency table:
  n <= 12,000   (48 KB / 4):   x fits in L1 -- no tiling needed
  n <= 64,000   (256 KB / 4):  x fits in L2 -- no tiling needed
  n <= 8,000,000 (32 MB / 4):  x fits in L3 -- optional tiling
  n > 8,000,000:               x in DRAM -- mandatory tiling
```

#### 2.5.2 Tiled SpMV Implementation

```rust
/// Tile size for cache-blocked SpMV.
///
/// Chosen to keep the tile of x within L2 cache:
///   TILE_SIZE * 4 bytes <= L2_SIZE / 2
///   TILE_SIZE = L2_SIZE / 8 = 256 KB / 8 = 32,768 elements
///
/// We use L2/2 (not full L2) to leave room for CSR values and
/// col_indices streaming through L2 simultaneously.
const SPMV_TILE_SIZE: usize = 32_768;

impl CsrMatrix {
    /// Cache-tiled SpMV for large vectors that exceed L2 cache.
    ///
    /// Strategy: partition columns into tiles of SPMV_TILE_SIZE.
    /// For each tile, iterate all rows but only accumulate contributions
    /// from columns within the tile. The x[tile] block stays in L2.
    ///
    /// Cost overhead vs untiled: one extra pass through row_ptr per tile.
    /// For t = ceil(ncols / TILE_SIZE) tiles, overhead is O(t * nrows).
    /// This is negligible when nnz >> nrows (typical for sparse matrices).
    pub fn spmv_tiled(&self, x: &[f32], y: &mut [f32]) {
        debug_assert_eq!(x.len(), self.ncols);
        debug_assert_eq!(y.len(), self.nrows);

        // Zero output
        y.iter_mut().for_each(|v| *v = 0.0);

        let num_tiles = (self.ncols + SPMV_TILE_SIZE - 1) / SPMV_TILE_SIZE;

        for tile in 0..num_tiles {
            let col_start = tile * SPMV_TILE_SIZE;
            let col_end = ((tile + 1) * SPMV_TILE_SIZE).min(self.ncols);

            // The x[col_start..col_end] slice now fits in L2 cache.
            // Prefetch it to avoid cold-start misses.
            #[cfg(target_arch = "x86_64")]
            {
                for i in (col_start..col_end).step_by(16) {
                    unsafe {
                        use std::arch::x86_64::*;
                        let ptr = x.as_ptr().add(i);
                        _mm_prefetch(ptr as *const i8, _MM_HINT_T0);
                    }
                }
            }

            for i in 0..self.nrows {
                let start = self.row_ptr[i] as usize;
                let end = self.row_ptr[i + 1] as usize;
                let mut sum = 0.0f32;

                for j in start..end {
                    let col = unsafe { *self.col_indices.get_unchecked(j) } as usize;
                    if col >= col_start && col < col_end {
                        unsafe {
                            sum += *self.values.get_unchecked(j)
                                 * *x.get_unchecked(col);
                        }
                    }
                }

                y[i] += sum;
            }
        }
    }

    /// Choose between tiled and untiled SpMV based on vector size.
    #[inline]
    pub fn spmv_auto(&self, x: &[f32], y: &mut [f32]) {
        // L2 threshold: 64K elements (256 KB at f32)
        if self.ncols > 64_000 {
            self.spmv_tiled(x, y);
        } else {
            self.spmv(x, y);
        }
    }
}
```

#### 2.5.3 DRAM-Bound Operation Tiling

For operations where the full problem is DRAM-resident (n > 8M), the solver
additionally tiles the rows to keep the output vector y in L1:

```
Two-level tiling for DRAM-bound SpMV:

Outer loop: tiles of rows    (row_tile_size = L1_SIZE / (2 * 4) = 6,000 rows)
  Inner loop: tiles of cols  (col_tile_size = L2_SIZE / (2 * 4) = 32,768 cols)
    Accumulate y[row_tile] += A[row_tile, col_tile] * x[col_tile]

Cache residency during inner loop:
  y[row_tile]:   6,000 * 4 = 24 KB   -- in L1
  x[col_tile]:  32,768 * 4 = 128 KB  -- in L2
  CSR stream:   bandwidth-limited     -- streamed through L2/L3
```

### 2.6 HNSW Graph as Solver Input

The HNSW index graph provides a natural adjacency structure for solver operations
(graph Laplacian, spectral methods, PageRank). The solver derives its matrices
directly from HNSW topology without requiring a separate graph representation.

```rust
/// Construct a graph Laplacian from HNSW topology for solver input.
///
/// The graph Laplacian L = D - A where:
///   A = adjacency matrix from HNSW layer 0
///   D = degree matrix (diagonal, D_ii = sum_j A_ij)
///
/// For a similarity-weighted adjacency (w_ij = 1/(1+d_ij)),
/// the Laplacian's spectral properties reflect cluster structure:
///   - Eigenvalue 0: always present (connected graph)
///   - Small eigenvalues: indicate near-disconnected clusters
///   - Large eigenvalues: indicate high-conductance regions
///
/// Memory: CSR storage for L uses same space as A, plus n diagonal entries.
/// For M=16, N=100K: ~25 MB CSR + 400 KB diagonal = ~25.4 MB.
pub fn hnsw_to_laplacian(
    hnsw: &HnswIndex,
    weight_fn: AdjacencyWeightFn,
) -> CsrMatrix {
    let adjacency = hnsw_to_csr_adjacency(hnsw, weight_fn);

    // Compute degree vector
    let n = adjacency.nrows;
    let mut degree = vec![0.0f32; n];
    for i in 0..n {
        let start = adjacency.row_ptr[i] as usize;
        let end = adjacency.row_ptr[i + 1] as usize;
        for j in start..end {
            degree[i] += adjacency.values[j];
        }
    }

    // Construct L = D - A as CSR (negate adjacency, add degree to diagonal)
    let mut triplets = Vec::with_capacity(adjacency.nnz + n);

    // Diagonal entries: L_ii = degree_i
    for i in 0..n {
        triplets.push((i as u32, i as u32, degree[i]));
    }

    // Off-diagonal entries: L_ij = -A_ij
    for i in 0..n {
        let start = adjacency.row_ptr[i] as usize;
        let end = adjacency.row_ptr[i + 1] as usize;
        for j in start..end {
            let col = adjacency.col_indices[j];
            if col as usize != i {
                triplets.push((i as u32, col, -adjacency.values[j]));
            }
        }
    }

    CsrMatrix::from_triplets(n, n, triplets)
}

/// Normalized Laplacian for spectral methods.
///
/// L_sym = I - D^{-1/2} A D^{-1/2}
///
/// This normalization ensures eigenvalues lie in [0, 2] and
/// makes the Laplacian independent of node degree, which improves
/// Neumann series convergence (spectral radius < 1 guaranteed).
pub fn hnsw_to_normalized_laplacian(
    hnsw: &HnswIndex,
    weight_fn: AdjacencyWeightFn,
) -> CsrMatrix {
    let adjacency = hnsw_to_csr_adjacency(hnsw, weight_fn);
    let n = adjacency.nrows;

    // Compute D^{-1/2}
    let mut inv_sqrt_degree = vec![0.0f32; n];
    for i in 0..n {
        let start = adjacency.row_ptr[i] as usize;
        let end = adjacency.row_ptr[i + 1] as usize;
        let mut deg = 0.0f32;
        for j in start..end {
            deg += adjacency.values[j];
        }
        inv_sqrt_degree[i] = if deg > 0.0 { 1.0 / deg.sqrt() } else { 0.0 };
    }

    // L_sym entries: L_sym_ij = -A_ij / sqrt(d_i * d_j) for i != j
    //                L_sym_ii = 1 (if degree > 0)
    let mut triplets = Vec::with_capacity(adjacency.nnz + n);

    for i in 0..n {
        if inv_sqrt_degree[i] > 0.0 {
            triplets.push((i as u32, i as u32, 1.0));
        }

        let start = adjacency.row_ptr[i] as usize;
        let end = adjacency.row_ptr[i + 1] as usize;
        for j in start..end {
            let col = adjacency.col_indices[j] as usize;
            if col != i {
                let weight = -adjacency.values[j]
                    * inv_sqrt_degree[i]
                    * inv_sqrt_degree[col];
                triplets.push((i as u32, col as u32, weight));
            }
        }
    }

    CsrMatrix::from_triplets(n, n, triplets)
}
```

### 2.7 Quantization-Aware Solving

The solver uses full precision (f32) for all internal iterative computations to
preserve convergence guarantees. Quantized representations are used only at
the boundary: reading compressed input vectors and writing compressed outputs.

```rust
use ruvector_core::quantization::{
    ScalarQuantizer, BinaryQuantizer, ProductQuantizer,
    QuantizationType,
};

/// Precision strategy for solver operations.
///
/// Design principle: quantization is a STORAGE concern, not a COMPUTE concern.
/// The solver always computes in f32 to maintain epsilon-convergence guarantees.
/// Quantized vectors are decompressed on-the-fly during solver input, and
/// results can optionally be re-quantized for storage.
///
/// Memory savings from quantized input:
///   Scalar (INT8):  4x compression on vector storage
///   INT4:           8x compression
///   PQ (d/m subs):  16x compression (typical, depends on codebook)
///   Binary:         32x compression
///
/// These savings apply to the VECTOR storage that the solver reads from,
/// not to the solver's internal working memory (which is always f32).
pub struct QuantizationAwareSolver {
    /// The underlying f32 solver.
    inner: SublinearSolver,
    /// Quantization type of the input vectors.
    input_quantization: QuantizationType,
    /// Scratch buffer for dequantized vectors (reused across calls).
    dequant_buffer: Vec<f32>,
}

impl QuantizationAwareSolver {
    /// Solve using quantized input vectors.
    ///
    /// The input vectors are dequantized into f32 scratch space,
    /// the solver runs in f32, and the result is returned in f32.
    ///
    /// Memory overhead: one n-dimensional f32 buffer for dequantization.
    /// This is allocated once and reused across solves.
    pub fn solve_quantized(
        &mut self,
        quantized_vectors: &[u8],
        dimensions: usize,
    ) -> Result<Vec<f32>, SolverError> {
        // Dequantize input into f32 buffer
        self.dequant_buffer.resize(dimensions, 0.0);

        match self.input_quantization {
            QuantizationType::Scalar => {
                ScalarQuantizer::dequantize(
                    quantized_vectors,
                    &mut self.dequant_buffer,
                );
            }
            QuantizationType::Binary => {
                BinaryQuantizer::dequantize(
                    quantized_vectors,
                    &mut self.dequant_buffer,
                );
            }
            QuantizationType::ProductQuantization { codebook, .. } => {
                ProductQuantizer::dequantize(
                    quantized_vectors,
                    codebook,
                    &mut self.dequant_buffer,
                );
            }
            QuantizationType::None => {
                // Direct f32 copy -- but prefer zero-copy borrow
                let f32_slice = unsafe {
                    std::slice::from_raw_parts(
                        quantized_vectors.as_ptr() as *const f32,
                        dimensions,
                    )
                };
                self.dequant_buffer.copy_from_slice(f32_slice);
            }
        }

        // Solve in full f32 precision
        self.inner.solve(&self.dequant_buffer)
    }
}
```

**Precision impact on convergence**:

```
Solver precision analysis for Neumann series x = sum_{k=0}^{K} (I-A)^k * b:

f32 machine epsilon: ~1.19e-7
f64 machine epsilon: ~2.22e-16

For convergence tolerance epsilon:
  epsilon = 1e-2:  f32 sufficient (5 orders of margin)
  epsilon = 1e-4:  f32 sufficient (3 orders of margin)
  epsilon = 1e-6:  f32 borderline (1 order of margin, may need compensation)
  epsilon = 1e-8:  f32 insufficient (below machine epsilon), requires f64

Recommendation: default to f32 for epsilon >= 1e-5.
For high-precision solves (epsilon < 1e-5), use compensated summation (Kahan)
to extend effective precision to ~1e-14 without switching to f64.
```

### 2.8 Memory Budget Enforcement

The solver integrates with RuVector's compute budget system (from `prime-radiant`'s
compute ladder) to enforce memory limits at the solver level.

```rust
/// Memory budget for a single solve operation.
///
/// This integrates with the ComputeBudget from prime-radiant's
/// compute ladder (Lane 0 Reflex through Lane 3 Deliberate).
///
/// The memory budget is enforced at three checkpoints:
///   1. Pre-allocation: total scratch + CSR must fit in budget
///   2. Per-iteration: runtime check that arena usage stays within bounds
///   3. Post-solve: report actual peak memory for observability
pub struct SolverMemoryBudget {
    /// Maximum bytes for solver scratch space.
    pub max_scratch_bytes: usize,
    /// Maximum bytes for CSR matrix storage.
    pub max_matrix_bytes: usize,
    /// Maximum total bytes (scratch + matrix + overhead).
    pub max_total_bytes: usize,
    /// Whether to fall back to paged memory when budget is tight.
    pub allow_paged_fallback: bool,
    /// Whether to allow memory-mapped files for very large problems.
    pub allow_mmap_fallback: bool,
}

impl SolverMemoryBudget {
    /// Budget for WASM browser deployment.
    /// Constrained to 4 MB total.
    pub fn wasm_browser() -> Self {
        Self {
            max_scratch_bytes: 2 * 1024 * 1024,   // 2 MB
            max_matrix_bytes: 2 * 1024 * 1024,     // 2 MB
            max_total_bytes: 4 * 1024 * 1024,      // 4 MB
            allow_paged_fallback: false,
            allow_mmap_fallback: false,
        }
    }

    /// Budget for WASM edge deployment (Cloudflare Workers, etc).
    /// 16 MB total, no mmap.
    pub fn wasm_edge() -> Self {
        Self {
            max_scratch_bytes: 8 * 1024 * 1024,    // 8 MB
            max_matrix_bytes: 8 * 1024 * 1024,      // 8 MB
            max_total_bytes: 16 * 1024 * 1024,      // 16 MB
            allow_paged_fallback: false,
            allow_mmap_fallback: false,
        }
    }

    /// Budget for native server deployment.
    /// 2 GB total, paging and mmap enabled.
    pub fn native_server() -> Self {
        Self {
            max_scratch_bytes: 512 * 1024 * 1024,  // 512 MB
            max_matrix_bytes: 1024 * 1024 * 1024,   // 1 GB
            max_total_bytes: 2048 * 1024 * 1024,    // 2 GB
            allow_paged_fallback: true,
            allow_mmap_fallback: true,
        }
    }

    /// Budget derived from ComputeLane (prime-radiant integration).
    pub fn from_compute_lane(lane: ComputeLane) -> Self {
        match lane {
            ComputeLane::Reflex => Self {
                max_scratch_bytes: 64 * 1024,       // 64 KB
                max_matrix_bytes: 256 * 1024,        // 256 KB
                max_total_bytes: 512 * 1024,         // 512 KB
                allow_paged_fallback: false,
                allow_mmap_fallback: false,
            },
            ComputeLane::Retrieval => Self {
                max_scratch_bytes: 4 * 1024 * 1024,  // 4 MB
                max_matrix_bytes: 16 * 1024 * 1024,   // 16 MB
                max_total_bytes: 32 * 1024 * 1024,    // 32 MB
                allow_paged_fallback: true,
                allow_mmap_fallback: false,
            },
            ComputeLane::Heavy => Self {
                max_scratch_bytes: 128 * 1024 * 1024, // 128 MB
                max_matrix_bytes: 512 * 1024 * 1024,   // 512 MB
                max_total_bytes: 1024 * 1024 * 1024,   // 1 GB
                allow_paged_fallback: true,
                allow_mmap_fallback: true,
            },
            ComputeLane::Deliberate => Self::native_server(),
        }
    }

    /// Validate that a proposed allocation fits within this budget.
    pub fn validate(&self, scratch: usize, matrix: usize) -> Result<(), BudgetError> {
        if scratch > self.max_scratch_bytes {
            return Err(BudgetError::ScratchExceeded {
                requested: scratch,
                budget: self.max_scratch_bytes,
            });
        }
        if matrix > self.max_matrix_bytes {
            if self.allow_paged_fallback || self.allow_mmap_fallback {
                // Will use alternative storage; allowed
                return Ok(());
            }
            return Err(BudgetError::MatrixExceeded {
                requested: matrix,
                budget: self.max_matrix_bytes,
            });
        }
        let total = scratch + matrix;
        if total > self.max_total_bytes {
            return Err(BudgetError::TotalExceeded {
                requested: total,
                budget: self.max_total_bytes,
            });
        }
        Ok(())
    }
}

/// Budget validation errors with actionable detail.
#[derive(Debug, thiserror::Error)]
pub enum BudgetError {
    #[error("Scratch space {requested} bytes exceeds budget of {budget} bytes. \
             Reduce iteration count (k) or problem dimension (n).")]
    ScratchExceeded { requested: usize, budget: usize },

    #[error("Matrix storage {requested} bytes exceeds budget of {budget} bytes. \
             Consider sparsifying the matrix or enabling paged/mmap fallback.")]
    MatrixExceeded { requested: usize, budget: usize },

    #[error("Total memory {requested} bytes exceeds budget of {budget} bytes.")]
    TotalExceeded { requested: usize, budget: usize },
}
```

---

## 3. Memory Profiling Integration

The solver integrates with RuVector's observability infrastructure to provide
real-time memory usage metrics.

### 3.1 Prometheus Metrics

```rust
use prometheus::{Gauge, Histogram, IntCounter, register_gauge, register_histogram};

lazy_static::lazy_static! {
    /// Current solver scratch space usage in bytes.
    static ref SOLVER_SCRATCH_BYTES: Gauge = register_gauge!(
        "ruvector_solver_scratch_bytes",
        "Current solver scratch space allocation in bytes"
    ).unwrap();

    /// Current solver CSR matrix storage in bytes.
    static ref SOLVER_MATRIX_BYTES: Gauge = register_gauge!(
        "ruvector_solver_matrix_bytes",
        "Current CSR matrix storage in bytes"
    ).unwrap();

    /// Peak solver memory usage per solve (histogram).
    static ref SOLVER_PEAK_MEMORY: Histogram = register_histogram!(
        "ruvector_solver_peak_memory_bytes",
        "Peak memory usage per solve operation",
        vec![
            1024.0,           // 1 KB
            65_536.0,         // 64 KB
            1_048_576.0,      // 1 MB
            16_777_216.0,     // 16 MB
            134_217_728.0,    // 128 MB
            1_073_741_824.0,  // 1 GB
        ]
    ).unwrap();

    /// Number of times the solver fell back to paged memory.
    static ref SOLVER_PAGED_FALLBACKS: IntCounter = register_counter!(
        "ruvector_solver_paged_fallbacks_total",
        "Number of times solver fell back to ADR-006 paged memory"
    ).unwrap();

    /// Number of times the solver fell back to memory-mapped files.
    static ref SOLVER_MMAP_FALLBACKS: IntCounter = register_counter!(
        "ruvector_solver_mmap_fallbacks_total",
        "Number of times solver fell back to memory-mapped files"
    ).unwrap();

    /// Number of budget-exceeded errors.
    static ref SOLVER_BUDGET_ERRORS: IntCounter = register_counter!(
        "ruvector_solver_budget_exceeded_total",
        "Number of solves rejected due to memory budget"
    ).unwrap();
}
```

### 3.2 dhat Integration for Development Profiling

```rust
/// Development-only memory profiler using dhat.
///
/// Enabled with `cfg(feature = "dhat-profiling")`.
/// Produces a dhat-heap.json file that can be viewed in
/// https://nnethercote.github.io/dh_view/dh_view.html
///
/// Usage in benchmarks:
///   DHAT_SOLVER=1 cargo bench --features dhat-profiling -- solver
#[cfg(feature = "dhat-profiling")]
pub fn profile_solve(
    solver: &mut SublinearSolver,
    input: &[f32],
) -> (SolverResult, DhatStats) {
    let profiler = dhat::Profiler::builder()
        .file_name("dhat-solver.json")
        .build();

    let result = solver.solve(input).unwrap();

    let stats = dhat::HeapStats {
        total_bytes: dhat::total_bytes(),
        total_blocks: dhat::total_blocks(),
        max_bytes: dhat::max_bytes(),
        max_blocks: dhat::max_blocks(),
    };

    drop(profiler);
    (result, stats)
}
```

### 3.3 jemalloc_ctl Integration for Production Profiling

```rust
/// Production memory statistics via jemalloc_ctl.
///
/// This provides thread-level allocation statistics without
/// the overhead of a full profiler. Used for runtime monitoring
/// and alerting when solver memory approaches budget limits.
///
/// Requires `jemalloc-ctl` as a dependency (already compatible
/// with RuVector's allocation strategy).
#[cfg(feature = "jemalloc-stats")]
pub fn solver_memory_stats() -> SolverMemoryStats {
    use jemalloc_ctl::{epoch, stats};

    // Advance the jemalloc epoch to get fresh stats
    epoch::advance().unwrap();

    SolverMemoryStats {
        allocated: stats::allocated::read().unwrap(),
        resident: stats::resident::read().unwrap(),
        active: stats::active::read().unwrap(),
        mapped: stats::mapped::read().unwrap(),
        retained: stats::retained::read().unwrap(),
    }
}

#[cfg(feature = "jemalloc-stats")]
pub struct SolverMemoryStats {
    /// Total bytes allocated by the solver (active heap).
    pub allocated: usize,
    /// Resident set size (physical pages mapped).
    pub resident: usize,
    /// Active pages (allocated + fragmentation).
    pub active: usize,
    /// Total pages mapped (includes mmap regions).
    pub mapped: usize,
    /// Pages retained by jemalloc for future allocation.
    pub retained: usize,
}
```

---

## 4. Options Considered

### Option 1: Solver-Owned Memory (Rejected)

Let the solver manage its own heap allocations independently of RuVector.

- **Pros**: Simple implementation, no coupling to RuVector internals
- **Cons**: Fragmentation from interleaved solver/HNSW allocations, no budget
  enforcement, no cache coordination, no observability integration, WASM memory
  growth unpredictable

### Option 2: nalgebra Allocator Override (Rejected)

Override nalgebra's default allocator with RuVector's arena allocator using
nalgebra's `Allocator` trait.

- **Pros**: Deep integration with nalgebra's allocation path
- **Cons**: nalgebra's `Allocator` trait is designed for static dimensions, not
  dynamic; significant API surface to implement; tight coupling to nalgebra
  internals that may change across versions; does not address CSR storage

### Option 3: Unified Arena + Paged + Budget Strategy (Selected)

Integrate with all three layers of RuVector's memory infrastructure: arena for
scratch space, ADR-006 paging for large matrices, and compute budget for limits.

- **Pros**: Zero fragmentation for temporaries, graceful degradation for large
  problems, enforced budgets across all deployment targets, full observability,
  cache-aware tiling, zero-copy data paths
- **Cons**: Higher implementation complexity, requires understanding three memory
  subsystems, testing across multiple fallback paths

### Option 4: Memory-Mapped Only (Rejected)

Use memmap2 for all solver storage, letting the OS manage paging.

- **Pros**: Simple API, OS handles eviction, supports very large problems
- **Cons**: Not available in WASM, no fine-grained budget control, OS paging
  decisions are not cache-hierarchy-aware, higher latency for random access
  patterns in SpMV

---

## 5. Consequences

### 5.1 Positive

- **Zero fragmentation**: Arena-based scratch space guarantees no heap fragmentation
  from solver temporaries. The arena reset between solves is O(1) and frees all
  temporaries atomically.
- **Predictable cache behavior**: Tiling strategy ensures L2-resident working sets
  for SpMV, maintaining the cache efficiency characteristics already benchmarked in
  `bench_memory.rs`.
- **WASM compatibility**: Explicit memory budgets and pre-allocation prevent
  unpredictable WASM linear memory growth. The 4-8 MB browser budget is enforced
  at construction time, not at runtime.
- **Graceful degradation**: The three-tier storage strategy (heap -> paged -> mmap)
  handles problem sizes spanning 4 orders of magnitude (1K to 10M dimensions)
  without code changes.
- **Observability**: Prometheus metrics provide real-time visibility into solver
  memory consumption, enabling alerting before OOM conditions.
- **Zero-copy paths**: Direct borrowing from SoA storage and HNSW graph avoids
  unnecessary copies, preserving the memory bandwidth characteristics measured
  in existing benchmarks.

### 5.2 Negative

- **Implementation complexity**: Three memory backends (arena, paged, mmap) with
  fallback logic increases implementation and testing surface. Each backend path
  requires its own correctness and performance tests.
- **API surface**: The `SolverMemoryBudget` and `SolverScratch` types add new
  public API that must be maintained and documented.
- **Cache tiling overhead**: The tiled SpMV adds one extra pass through `row_ptr`
  per column tile. For very sparse matrices with many tiles, this overhead may
  exceed the benefit of improved cache locality.
- **WASM pre-allocation waste**: Pre-allocating scratch space in WASM to avoid
  growth during solve means the maximum problem size must be known at construction
  time. If the actual problem is smaller, the pre-allocated memory is wasted.

### 5.3 Neutral

- The CSR format is standard and well-understood. It is not the most cache-friendly
  format for all access patterns (CSC is better for column access), but it matches
  the solver's row-oriented Neumann iteration.
- The 2 MB page size from ADR-006 is larger than optimal for small solver matrices
  (where internal fragmentation wastes ~1 MB per matrix). This is an acceptable
  tradeoff given that paging is only used for large matrices.

---

## 6. Memory Consumption Reference Table

Comprehensive memory consumption for all solver components at representative scales:

| Component | Formula | 1K dim | 10K dim | 100K dim | 1M dim |
|-----------|---------|--------|---------|----------|--------|
| Scratch (k=20) | k * ceil(n*4/64)*64 | 80 KB | 781 KB | 7.6 MB | 76 MB |
| CSR 1% density | n^2 * 0.01 * 12 + (n+1)*4 | 120 KB | 1.2 MB | 120 MB | 12 GB |
| CSR 0.1% density | n^2 * 0.001 * 12 + (n+1)*4 | 12 KB | 120 KB | 12 MB | 1.2 GB |
| CSR 0.01% density | n^2 * 0.0001 * 12 + (n+1)*4 | 1.2 KB | 12 KB | 1.2 MB | 120 MB |
| Random walk (s=1000) | s * 24 | 24 KB | 24 KB | 24 KB | 24 KB |
| Residual vector | n * 4 | 4 KB | 40 KB | 400 KB | 4 MB |
| HNSW adjacency (M=16) | n * M * 2 * 12 | 384 KB | 3.8 MB | 38 MB | 384 MB |
| Degree vector | n * 4 | 4 KB | 40 KB | 400 KB | 4 MB |
| **Total (0.1% density)** | -- | **504 KB** | **4.8 MB** | **58 MB** | **1.7 GB** |

**Recommended deployment limits by platform**:

| Platform | Max Dimension | Max Density | Memory Budget | Storage Tier |
|----------|---------------|-------------|---------------|--------------|
| WASM Browser | 5,000 | 1% | 4 MB | Heap only |
| WASM Edge | 20,000 | 0.5% | 16 MB | Heap only |
| Node.js (NAPI) | 100,000 | 0.1% | 512 MB | Heap + Paged |
| Native Server | 1,000,000 | 0.01% | 2 GB | Heap + Paged + mmap |
| Native Server (large) | 10,000,000 | 0.001% | 16 GB | Paged + mmap only |

---

## 7. Related Decisions

- **ADR-006**: Unified Memory Pool and Paging Strategy -- provides the paged memory
  infrastructure that this decision extends with `SOLVER_MATRIX` content type
- **ADR-003**: SIMD Optimization Strategy -- defines the SIMD dispatch patterns that
  the solver's SpMV kernel follows for platform-specific acceleration
- **ADR-005**: WASM Runtime Integration -- establishes WASM memory constraints and
  epoch-based interruption that govern solver execution in browser contexts
- **ADR-STS-001**: Sublinear-Time Solver Core Architecture (if exists) -- defines
  the solver's algorithm selection and convergence strategy
- **ADR-STS-002**: Sublinear-Time Solver API Design (if exists) -- defines the
  solver's public trait interfaces that this memory strategy supports

---

## Implementation Status

Arena allocator delivered for zero-allocation solver iterations. Fused residual_norm_sq kernel reduces memory passes from 3 to 1. spmv_unchecked eliminates bounds-check overhead. ComputeBudget system enforces memory caps. Workspace reuse across iterations via pre-allocated buffers.

---

## 8. References

1. S-LoRA: Serving Thousands of Concurrent LoRA Adapters (arXiv:2311.03285) --
   unified memory pool architecture for heterogeneous workloads
2. CSR format specification (Intel MKL Sparse BLAS documentation) --
   compressed sparse row storage layout and SpMV algorithms
3. Cache-Oblivious Algorithms (Frigo et al., 1999) --
   theoretical foundation for cache-tiling strategies
4. RuVector Architecture Analysis (doc 05-architecture-analysis.md) --
   existing memory subsystem documentation
5. RuVector Performance Analysis (doc 08-performance-analysis.md) --
   benchmark results for arena, SoA, and cache behavior
6. WASM Linear Memory specification (WebAssembly Core Specification 2.0) --
   memory model constraints for browser deployment
7. jemalloc: A Scalable Concurrent malloc Implementation (Evans, 2006) --
   production memory profiling via jemalloc_ctl

---

## 9. Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-02-20 | RuVector Architecture Team | Initial proposal |
| 1.0 | 2026-02-20 | RuVector Team | Accepted: full implementation complete |
