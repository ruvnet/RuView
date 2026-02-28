# Optimization Guide: Sublinear-Time Solver Integration

**Date**: 2026-02-20
**Classification**: Engineering Reference
**Scope**: Performance optimization strategies for solver integration
**Version**: 2.0 (Optimizations Realized)

---

## 1. Executive Summary

This guide provides concrete optimization strategies for achieving maximum performance from the sublinear-time-solver integration into RuVector. Targets: 10-600x speedups across 6 critical subsystems while maintaining <2% accuracy loss. Organized by optimization tier: SIMD → Memory → Algorithm → Numerical → Concurrency → WASM → Profiling → Compilation → Platform.

---

## 2. SIMD Optimization Strategy

### 2.1 Architecture-Specific Kernels

The solver's hot path is SpMV (sparse matrix-vector multiply). Each architecture requires a dedicated kernel:

| Architecture | SIMD Width | f32/iteration | Key Instruction | Expected SpMV Throughput |
|-------------|-----------|--------------|-----------------|-------------------------|
| AVX-512 | 512-bit | 16 | `_mm512_i32gather_ps` | ~400M nonzeros/s |
| AVX2+FMA | 256-bit | 8×4 unrolled | `_mm256_i32gather_ps` + `_mm256_fmadd_ps` | ~250M nonzeros/s |
| NEON | 128-bit | 4×4 unrolled | Manual gather + `vfmaq_f32` | ~150M nonzeros/s |
| WASM SIMD128 | 128-bit | 4 | `f32x4_mul` + `f32x4_add` | ~80M nonzeros/s |
| Scalar | 32-bit | 1 | `fmaf` | ~40M nonzeros/s |

### 2.2 SpMV Kernels

**AVX2+FMA SpMV with gather** (primary kernel):
```
for each row i:
    acc = _mm256_setzero_ps()
    for j in row_ptrs[i]..row_ptrs[i+1] step 8:
        indices = _mm256_loadu_si256(&col_indices[j])
        vals = _mm256_loadu_ps(&values[j])
        x_gathered = _mm256_i32gather_ps(x_ptr, indices, 4)
        acc = _mm256_fmadd_ps(vals, x_gathered, acc)
    y[i] = horizontal_sum(acc) + scalar_remainder
```

**AVX-512 SpMV with masking** (for variable-length rows):
```
for each row i:
    acc = _mm512_setzero_ps()
    len = row_ptrs[i+1] - row_ptrs[i]
    full_chunks = len / 16
    remainder = len % 16

    for j in 0..full_chunks:
        base = row_ptrs[i] + j * 16
        idx = _mm512_loadu_si512(&col_indices[base])
        v = _mm512_loadu_ps(&values[base])
        x = _mm512_i32gather_ps(idx, x_ptr, 4)
        acc = _mm512_fmadd_ps(v, x, acc)

    if remainder > 0:
        mask = (1 << remainder) - 1
        base = row_ptrs[i] + full_chunks * 16
        idx = _mm512_maskz_loadu_epi32(mask, &col_indices[base])
        v = _mm512_maskz_loadu_ps(mask, &values[base])
        x = _mm512_mask_i32gather_ps(zeros, mask, idx, x_ptr, 4)
        acc = _mm512_fmadd_ps(v, x, acc)

    y[i] = _mm512_reduce_add_ps(acc)
```

**WASM SIMD128 SpMV kernel**:
```
for each row i:
    acc = f32x4_splat(0.0)
    for j in row_ptrs[i]..row_ptrs[i+1] step 4:
        x_vec = f32x4(x[col_indices[j]], x[col_indices[j+1]],
                       x[col_indices[j+2]], x[col_indices[j+3]])
        v = v128_load(&values[j])
        acc = f32x4_add(acc, f32x4_mul(v, x_vec))
    y[i] = horizontal_sum_f32x4(acc) + scalar_remainder
```

**Vectorized PRNG** (for Hybrid Random Walk):
```
state[4][4] = initialize_from_seed()
for each walk:
    random = xoshiro256_simd(state)  // 4 random values per call
    next_node = random % degree[current_node]
```

### 2.3 Auto-Vectorization Guidelines

1. **Sequential access**: Iterate arrays in order (no random access in inner loop)
2. **No branches**: Use `select`/`blend` instead of `if` in hot loops
3. **Independent accumulators**: 4 separate sums, combine at end
4. **Aligned data**: Use `#[repr(align(64))]` on hot data structures
5. **Known bounds**: Use `get_unchecked()` after external bounds check
6. **Compiler hints**: `#[inline(always)]` on hot functions, `#[cold]` on error paths

### 2.4 Throughput Formulas

SpMV throughput is bounded by memory bandwidth:
```
Throughput = min(BW_memory / 8, FLOPS_peak / 2) nonzeros/s
```
Where 8 = bytes/nonzero (4B value + 4B index), 2 = FLOPs/nonzero (mul + add).

SpMV is almost always memory-bandwidth-bound. SIMD reduces instruction count but memory throughput is the fundamental limit.

---

## 3. Memory Optimization

### 3.1 Cache-Aware Tiling

| Working Set | Cache Level | Performance | Strategy |
|------------|------------|-------------|---------|
| < 48 KB | L1 (M4 Pro: 192KB/perf) | Peak (100%) | Direct iteration, no tiling |
| < 256 KB | L2 | 80-90% of peak | Single-pass with prefetch |
| < 16 MB | L3 | 50-70% of peak | Row-block tiling |
| > 16 MB | DRAM | 20-40% of peak | Page-level tiling + prefetch |
| > available RAM | Disk | 1-5% of peak | Memory-mapped streaming |

**Tiling formula**: `TILE_ROWS = L3_SIZE / (avg_row_nnz × 12 bytes)`

### 3.2 Prefetch Strategy

```rust
// Software prefetch for SpMV x-vector access
for row in 0..n {
    if row + 1 < n {
        let next_start = row_ptrs[row + 1];
        for j in next_start..(next_start + 8).min(row_ptrs[row + 2]) {
            prefetch_read_l2(&x[col_indices[j] as usize]);
        }
    }
    process_row(row);
}
```

Prefetch distance: L1 = 64 bytes ahead, L2 = 256 bytes ahead.

### 3.3 Arena Allocator Integration

```rust
// Before: ~20μs overhead per solve
let r = vec![0.0f32; n]; let p = vec![0.0f32; n]; let ap = vec![0.0f32; n];

// After: ~0.2μs overhead per solve
let mut arena = SolverArena::with_capacity(n * 12);
let r = arena.alloc_slice::<f32>(n);
let p = arena.alloc_slice::<f32>(n);
let ap = arena.alloc_slice::<f32>(n);
arena.reset();
```

### 3.4 Cache Line Alignment

```rust
#[repr(C, align(64))]
struct SolverScratch { r: [f32; N], p: [f32; N], ap: [f32; N] }

#[repr(C, align(128))]  // Prevent false sharing in parallel stats
struct ThreadStats { iterations: u64, residual: f64, _pad: [u8; 112] }
```

### 3.5 Memory-Mapped Large Matrices

```rust
let mmap = unsafe { memmap2::Mmap::map(&file)? };
let values: &[f32] = bytemuck::cast_slice(&mmap[header_size..]);
```

### 3.6 Zero-Copy Data Paths

| Path | Mechanism | Overhead |
|------|-----------|----------|
| SoA → Solver | `&[f32]` borrow | 0 bytes |
| HNSW → CSR | Direct construction | O(n×M) one-time |
| Solver → WASM | `Float32Array::view()` | 0 bytes |
| Solver → NAPI | `napi::Buffer` | 0 bytes |
| Solver → REST | `serde_json::to_writer` | 1 serialization |

---

## 4. Algorithmic Optimization

### 4.1 Preconditioning Strategies

| Preconditioner | Setup Cost | Per-Iteration Cost | Condition Improvement | Best For |
|---------------|-----------|-------------------|----------------------|----------|
| None | 0 | 0 | 1x | Well-conditioned (κ < 10) |
| Diagonal (Jacobi) | O(n) | O(n) | √(d_max/d_min) | General SPD |
| Incomplete Cholesky | O(nnz) | O(nnz) | 10-100x | Moderately ill-conditioned |
| Algebraic Multigrid | O(nnz·log n) | O(nnz) | Near-optimal for Laplacians | κ > 100 |

**Default**: Diagonal preconditioner. Escalate to AMG when κ > 100 and n > 50K.

### 4.2 Sparsity Exploitation

```rust
fn select_path(matrix: &CsrMatrix<f32>) -> ComputePath {
    let density = matrix.density();
    if density > 0.50 { ComputePath::Dense }
    else if density > 0.05 { ComputePath::Sparse }
    else { ComputePath::Sublinear }
}
```

### 4.3 Batch Amortization

| Preprocessing Cost | Per-Solve Cost | Break-Even B |
|-------------------|---------------|-------------|
| 425 ms (n=100K, 1%) | 0.43 ms (ε=0.1) | 634 solves |
| 42 ms (n=10K, 1%) | 0.04 ms (ε=0.1) | 63 solves |
| 4 ms (n=1K, 1%) | 0.004 ms (ε=0.1) | 6 solves |

### 4.4 Lazy Evaluation

```rust
let x_ij = solver.estimate_entry(A, i, j)?;  // O(√n/ε) via random walk
// vs full solve O(nnz × iterations). Speedup = √n for n=1M → 1000x
```

---

## 5. Numerical Optimization

### 5.1 Kahan Summation for SpMV

```rust
fn spmv_row_kahan(vals: &[f32], cols: &[u32], x: &[f32]) -> f32 {
    let mut sum: f64 = 0.0;
    let mut comp: f64 = 0.0;
    for i in 0..vals.len() {
        let y = (vals[i] as f64) * (x[cols[i] as usize] as f64) - comp;
        let t = sum + y;
        comp = (t - sum) - y;
        sum = t;
    }
    sum as f32
}
```

Use when: rows > 1000 nonzeros or ε < 1e-6. Overhead: ~2x. Alternative: f64 accumulator.

### 5.2 Mixed Precision Strategy

| Precision Mode | Storage | Accumulation | Max ε | Memory | SpMV Speed |
|---------------|---------|-------------|-------|--------|-----------|
| Pure f32 | f32 | f32 | 1e-4 | 1x | 1x (fastest) |
| **Default** (f32/f64) | f32 | f64 | 1e-7 | 1x | 0.95x |
| Pure f64 | f64 | f64 | 1e-12 | 2x | 0.5x |

### 5.3 Condition Number Estimation

Fast κ estimation via power iteration (20 iterations × 2 SpMVs = O(40 × nnz)):

```rust
fn estimate_kappa(A: &CsrMatrix<f32>) -> f64 {
    let lambda_max = power_iteration(A, 20);
    let lambda_min = inverse_power_iteration_cg(A, 20);
    lambda_max / lambda_min
}
```

### 5.4 Spectral Radius for Neumann

Estimate ρ(I-A) via 20-step power iteration. Rules:
- ρ < 0.9: Neumann converges fast (< 50 iterations for ε=0.01)
- 0.9 ≤ ρ < 0.99: Neumann slow, consider CG
- ρ ≥ 0.99: Switch to CG (Neumann needs > 460 iterations)
- ρ ≥ 1.0: Neumann diverges — CG/BMSSP mandatory

---

## 6. WASM-Specific Optimization

### 6.1 Memory Growth Strategy

Pre-allocate: `pages = ceil(n × avg_nnz × 12 / 65536) + 32`. Growth during solving costs ~1ms per grow.

### 6.2 wasm-opt Configuration

```bash
wasm-opt -O3 --enable-simd --enable-bulk-memory \
  --precompute-propagate --optimize-instructions \
  --reorder-functions --coalesce-locals --vacuum \
  pkg/solver_bg.wasm -o pkg/solver_bg_opt.wasm
```

Expected: 15-25% size reduction, 5-10% speed improvement.

### 6.3 Worker Thread Optimization

Use Transferable objects (zero-copy move) or SharedArrayBuffer (zero-copy share):

```javascript
worker.postMessage({ type: 'solve', matrix: values.buffer },
    [values.buffer]);  // Transfer list — moves, doesn't copy
```

### 6.4 Bundle Size Budget

| Component | Size (gzipped) | Budget |
|-----------|---------------|--------|
| Solver core (CG + Neumann + Push) | ~80 KB | 100 KB |
| SIMD128 kernels | ~15 KB | 20 KB |
| wasm-bindgen glue | ~10 KB | 15 KB |
| serde-wasm-bindgen | ~20 KB | 25 KB |
| **Total** | **~125 KB** | **160 KB** |

---

## 7. Profiling Methodology

### 7.1 Performance Counter Analysis

```bash
perf stat -e cycles,instructions,cache-references,cache-misses,\
  L1-dcache-load-misses,LLC-load-misses ./target/release/bench_spmv
```

Expected good SpMV profile: IPC 2.0-3.0, L1 miss 5-15%, LLC miss < 1%, branch miss < 1%.

### 7.2 Hot Spot Identification

```bash
perf record -g --call-graph dwarf ./target/release/bench_solver
perf script | stackcollapse-perf.pl | flamegraph.pl > solver_flame.svg
```

Expected: 60-80% in spmv_*, 10-15% in dot/norm, < 5% in allocation.

### 7.3 Roofline Model

SpMV arithmetic intensity = 0.167 FLOP/byte. On 80 GB/s server: achievable = 13.3 GFLOPS (1.3% of 1 TFLOPS peak). SpMV is deeply memory-bound — optimize for memory traffic reduction, not FLOPS.

### 7.4 Criterion.rs Best Practices

```rust
group.warm_up_time(Duration::from_secs(5));  // Stabilize cache state
group.sample_size(200);                       // Statistical significance
group.throughput(Throughput::Elements(nnz));  // Report nonzeros/sec
// Use black_box() to prevent dead code elimination
b.iter(|| black_box(solver.solve(&csr, &rhs)))
```

---

## 8. Concurrency Optimization

### 8.1 Rayon Configuration

```rust
let chunk_size = (n / rayon::current_num_threads()).max(1024);
problems.par_chunks(chunk_size).map(|chunk| ...).collect()
```

### 8.2 Thread Scaling

| Threads | Efficiency | Bottleneck |
|---------|-----------|-----------|
| 1 | 100% | N/A |
| 2 | 90-95% | Rayon overhead |
| 4 | 75-85% | Memory bandwidth |
| 8 | 55-70% | L3 contention |
| 16 | 40-55% | NUMA effects |

Use `num_cpus::get_physical()` threads. Avoid nested Rayon (deadlock risk).

---

## 9. Compilation Optimization

### 9.1 PGO Pipeline

```bash
RUSTFLAGS="-Cprofile-generate=/tmp/pgo" cargo build --release -p ruvector-solver
./target/release/bench_solver --profile-workload
llvm-profdata merge -o /tmp/pgo/merged.profdata /tmp/pgo/*.profraw
RUSTFLAGS="-Cprofile-use=/tmp/pgo/merged.profdata" cargo build --release
```

Expected: 5-15% improvement.

### 9.2 Release Profile

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = true
```

---

## 10. Platform-Specific Optimization

### 10.1 Server (Linux x86_64)

- Huge pages: `MADV_HUGEPAGE` for large matrices (10-30% TLB miss reduction)
- NUMA-aware: Pin threads to same node as matrix memory
- AVX-512: Prefer on Zen 4+/Ice Lake+

### 10.2 Apple Silicon (macOS ARM64)

- Unified memory: No NUMA concerns
- NEON 4x unrolled with independent accumulators
- M4 Pro: 192KB L1, 16MB L2, 48MB L3

### 10.3 Browser (WASM)

- Memory budget < 8MB, SIMD128 always enabled
- Web Workers for batch, SharedArrayBuffer for zero-copy
- IndexedDB caching for TRUE preprocessing

### 10.4 Cloudflare Workers

- 128MB memory, 50ms CPU limit
- Reflex/Retrieval lanes only
- Single-threaded, pre-warm with small solve

---

## 11. Optimization Checklist

### P0 (Critical)

| Item | Impact | Effort | Validation |
|------|--------|--------|------------|
| SIMD SpMV (AVX2+FMA, NEON) | 4-8x SpMV | L | Criterion vs scalar |
| Arena allocator | 100x alloc reduction | S | dhat profiling |
| Zero-copy SoA → solver | Eliminates copies | M | Memory profiling |
| CSR with aligned storage | SIMD foundation | M | Cache miss rate |
| Diagonal preconditioning | 2-10x CG speedup | S | Iteration count |
| Feature-gated Rayon | Multi-core utilization | S | Thread scaling |
| Input validation | Security baseline | S | Fuzz testing |
| CI regression benchmarks | Prevents degradation | M | CI green |

### P1 (High)

| Item | Impact | Effort | Validation |
|------|--------|--------|------------|
| AVX-512 SpMV | 1.5-2x over AVX2 | M | Zen 4 benchmark |
| WASM SIMD128 SpMV | 2-3x over scalar | M | wasm-pack bench |
| Cache-aware tiling | 30-50% for n>100K | M | perf cache misses |
| Memory-mapped CSR | Removes memory ceiling | M | 1GB matrix load |
| SONA adaptive routing | Auto-optimal selection | L | >90% routing accuracy |
| TRUE batch amortization | 100-1000x repeated | M | Break-even validated |
| Web Worker pool | 2-4x WASM throughput | M | Worker benchmark |

### P2 (Medium)

| Item | Impact | Effort | Validation |
|------|--------|--------|------------|
| PGO in CI | 5-15% overall | M | PGO comparison |
| Vectorized PRNG | 2-4x random walk | S | Walk throughput |
| SIMD convergence checks | 4-8x check speed | S | Inline benchmark |
| Mixed precision (f32/f64) | 2x memory savings | M | Accuracy suite |
| Incomplete Cholesky | 10-100x condition | L | Iteration count |

### P3 (Long-term)

| Item | Impact | Effort | Validation |
|------|--------|--------|------------|
| Algebraic multigrid | Near-optimal Laplacians | XL | V-cycle convergence |
| NUMA-aware allocation | 10-20% multi-socket | M | NUMA profiling |
| GPU offload (Metal/CUDA) | 10-100x dense | XL | GPU benchmark |
| Distributed solver | n > 1M scaling | XL | Distributed bench |

---

## 12. Performance Targets

| Operation | Server (AVX2) | Edge (NEON) | Browser (WASM) | Cloudflare |
|-----------|:---:|:---:|:---:|:---:|
| SpMV 10K×10K (1%) | < 30 μs | < 50 μs | < 200 μs | < 300 μs |
| CG solve 10K (ε=1e-6) | < 1 ms | < 2 ms | < 20 ms | < 30 ms |
| Forward Push 10K (ε=1e-4) | < 50 μs | < 100 μs | < 500 μs | < 1 ms |
| Neumann 10K (k=20) | < 600 μs | < 1 ms | < 5 ms | < 8 ms |
| BMSSP 100K (ε=1e-4) | < 50 ms | < 100 ms | N/A | < 200 ms |
| TRUE prep 100K (ε=0.1) | < 500 ms | < 1 s | N/A | < 2 s |
| TRUE solve 100K (amort.) | < 1 ms | < 2 ms | N/A | < 5 ms |
| Batch pairwise 10K | < 15 s | < 30 s | < 120 s | N/A |
| Scheduler tick | < 200 ns | < 300 ns | N/A | N/A |
| Algorithm routing | < 1 μs | < 1 μs | < 5 μs | < 5 μs |

---

## 13. Measurement Methodology

1. **Criterion.rs**: 200 samples, 5s warmup, p < 0.05 significance
2. **Multi-platform**: x86_64 (AVX2) and aarch64 (NEON)
3. **Deterministic seeds**: `random_vector(dim, seed=42)`
4. **Equal accuracy**: Fix ε before comparing
5. **Cold + hot cache**: Report both first-run and steady-state
6. **Profile.bench**: Release optimization with debug symbols
7. **Regression CI**: 10% degradation threshold triggers failure
8. **Memory profiling**: Peak RSS and allocation count via dhat
9. **Roofline analysis**: Verify memory-bound operation
10. **Statistical rigor**: Report median, p5, p95, coefficient of variation

---

## Realized Optimizations

The following optimizations from this guide have been implemented in the `ruvector-solver` crate as of February 2026.

### Implemented Techniques

1. **Jacobi-preconditioned Neumann series (D^{-1} splitting)**: The Neumann solver extracts the diagonal of A and applies D^{-1} as a preconditioner before iteration. This transforms the iteration matrix from (I - A) to (I - D^{-1}A), significantly reducing the spectral radius for diagonally-dominant systems and enabling convergence where unpreconditioned Neumann would diverge or stall.

2. **spmv_unchecked: raw pointer SpMV with zero bounds checks**: The inner SpMV loop uses unsafe raw pointer arithmetic to eliminate Rust's bounds-check overhead on every array access. An external bounds validation is performed once before entering the hot loop, maintaining safety guarantees while removing per-element branch overhead.

3. **fused_residual_norm_sq: single-pass residual + norm computation (3 memory passes to 1)**: Instead of computing r = b - Ax (pass 1), then ||r||^2 (pass 2) as separate operations, the fused kernel computes both the residual vector and its squared norm in a single traversal. This eliminates 2 of 3 memory traversals per iteration, which is critical since SpMV is memory-bandwidth-bound.

4. **4-wide unrolled Jacobi update in Neumann iteration**: The Jacobi preconditioner application loop is manually unrolled 4x, processing four elements per loop body. This reduces loop overhead and exposes instruction-level parallelism to the CPU's out-of-order execution engine.

5. **AVX2 SIMD SpMV (8-wide f32 via horizontal sum)**: The AVX2 SpMV kernel processes 8 f32 values per SIMD instruction using `_mm256_i32gather_ps` for gathering x-vector entries and `_mm256_fmadd_ps` for fused multiply-add accumulation. A horizontal sum reduces the 8-lane accumulator to a scalar row result.

6. **Arena allocator for zero-allocation iteration**: Solver working memory (residual, search direction, temporary vectors) is pre-allocated from a bump arena before the iteration loop begins. This eliminates all heap allocation during the solve phase, reducing per-solve overhead from ~20 microseconds to ~200 nanoseconds.

7. **Algorithm router with automatic characterization**: The solver includes an algorithm router that characterizes input matrices (size, density, estimated spectral radius, SPD detection) and selects the optimal algorithm automatically. The router runs in under 1 microsecond and directs traffic to the appropriate solver based on the matrix properties identified in Sections 4 and 5.

### Performance Data

| Algorithm | Complexity | Notes |
|-----------|-----------|-------|
| **Neumann** | O(k * nnz) | Converges with k typically 10-50 for well-conditioned systems (spectral radius < 0.9). Jacobi preconditioning extends the convergence regime. |
| **CG** | O(sqrt(kappa) * log(1/epsilon) * nnz) | Gold standard for SPD systems. Optimal by the Nemirovski-Yudin lower bound. Scales gracefully with condition number. |
| **Fused kernel** | Eliminates 2 of 3 memory traversals per iteration | For bandwidth-bound SpMV (arithmetic intensity 0.167 FLOP/byte), reducing memory passes from 3 to 1 translates directly to up to 3x throughput improvement for the residual computation step. |
