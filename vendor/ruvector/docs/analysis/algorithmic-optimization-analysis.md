# Algorithmic Optimization Analysis: Mincut-Gated Transformer

**Analysis Date**: 2025-12-26
**Crate**: `/home/user/ruvector/crates/ruvector-mincut-gated-transformer`
**Focus Files**: `spectral.rs`, `sparse_attention.rs`, `early_exit.rs`, `mod_routing.rs`

---

## Executive Summary

Found **11 high-impact optimization opportunities** with potential for:
- **90% reduction** in eigenvector computation time (sparse matrices)
- **50% reduction** in sparse attention mask building (hash-based deduplication)
- **60% reduction** in top-k computation (heap-based selection)
- **Elimination** of redundant lambda stability calculations

---

## 1. src/spectral.rs - Eigenvector Computation

### CRITICAL: Sparse Matrix Representation (O(n²) → O(E))

**File**: `src/spectral.rs`
**Lines**: 318-326, 350-356

**Issue**: Graph Laplacian is treated as dense matrix (n×n), but it's inherently sparse (only edges have non-zero values).

```rust
// CURRENT: O(n²) per iteration
for i in 0..n {
    let mut sum = 0.0f32;
    for j in 0..n {
        sum += matrix[i * n + j] * v[j];  // ← Iterates all n² entries
    }
    v_new[i] = sum;
}
```

**Expected Complexity**:
- Current: O(k × iters × n²) for k eigenvectors
- Optimized: O(k × iters × E) where E = number of edges

**Optimization**:
```rust
// OPTIMIZED: CSR (Compressed Sparse Row) format
struct SparseMatrix {
    row_ptr: Vec<usize>,    // Size: n+1
    col_idx: Vec<usize>,    // Size: nnz (non-zeros)
    values: Vec<f32>,       // Size: nnz
}

// O(E) matrix-vector multiplication
fn sparse_matvec(matrix: &SparseMatrix, v: &[f32], result: &mut [f32]) {
    for i in 0..matrix.row_ptr.len() - 1 {
        let mut sum = 0.0;
        for j in matrix.row_ptr[i]..matrix.row_ptr[i + 1] {
            sum += matrix.values[j] * v[matrix.col_idx[j]];
        }
        result[i] = sum;
    }
}
```

**Impact**: For typical graphs with E << n², this is **10-100x faster**.

**Example**: For n=1000 tokens, E=5000 edges:
- Dense: 1M operations per iteration
- Sparse: 5K operations per iteration (**200x speedup**)

---

### HIGH: Deflation Algorithm Inefficiency (O(k×n²) → O(k×n×iters))

**File**: `src/spectral.rs`
**Lines**: 176-184

**Issue**: Computing k eigenvectors using deflation requires k separate power iterations with matrix updates.

```rust
// CURRENT: Deflate after each eigenvector
for _ in 0..k {
    let evec = power_iteration(&shifted, n, 100);
    let eigenvalue = rayleigh_quotient(&shifted, n, &evec);

    // O(n²) deflation: A := A - λ * v * v^T
    for i in 0..n {
        for j in 0..n {
            shifted[i * n + j] -= eigenvalue * evec[i] * evec[j];  // ← Full matrix update
        }
    }
}
```

**Optimization**: Use **Lanczos algorithm** instead of deflated power iteration.

**Algorithm**:
```rust
// Lanczos tridiagonalization: O(m × E) where m = Lanczos steps
// Produces tridiagonal matrix T that captures dominant eigenspace
// Then solve T's eigenvalues/eigenvectors (O(m³) but m << n)

fn lanczos_eigenvectors(laplacian_edges: &[(u16, u16)], n: usize, k: usize) -> Vec<Vec<f32>> {
    const M: usize = 50; // Lanczos iterations (tune based on k)
    let m = (k * 3).min(M);

    // Build tridiagonal matrix via Lanczos
    let (alpha, beta) = lanczos_tridiagonalize(laplacian_edges, n, m);

    // Solve small tridiagonal eigenvalue problem: O(m³)
    let (evals, evecs_small) = tridiag_eigen(&alpha, &beta, k);

    // Project back to full space: O(m × n)
    project_eigenvectors(&evecs_small, n, k)
}
```

**Expected Complexity**:
- Current: O(k × iters × n²) = O(k × 100 × n²)
- Lanczos: O(m × E + m³) ≈ O(50 × E + 50³) where m ≈ 3k

**Impact**: For n=500, k=8, E=2500:
- Current: 8 × 100 × 250K = **200M operations**
- Lanczos: 50 × 2.5K + 125K = **250K operations** (**800x speedup**)

**Mathematical Foundation**: Lanczos method from Golub & Van Loan "Matrix Computations" (3rd ed, §9.3).

---

### MEDIUM: Redundant Matrix-Vector Product

**File**: `src/spectral.rs`
**Lines**: 173, 177, 350-356

**Issue**: `rayleigh_quotient` recomputes A×v even though it was just computed in the final power iteration.

```rust
// Line 173: Last iteration computes A×v
let evec = power_iteration(&shifted, n, 100);  // ← Computes A×v internally

// Line 177: Immediately recomputes A×v
let eigenvalue = rayleigh_quotient(&shifted, n, &evec);  // ← Redundant A×v
```

**Optimization**: Return both eigenvector and A×v from power iteration.

```rust
fn power_iteration_with_av(matrix: &[f32], n: usize, num_iters: u16)
    -> (Vec<f32>, Vec<f32>) // Returns (v, A×v)
{
    // ... iterations ...

    // Last iteration: compute and save A×v
    let mut av = vec![0.0f32; n];
    for i in 0..n {
        let mut sum = 0.0;
        for j in 0..n {
            sum += matrix[i * n + j] * v[j];
        }
        av[i] = sum;
    }

    // Normalize v
    let norm: f32 = av.iter().map(|x| x * x).sum::<f32>().sqrt();
    for x in &mut av { *x /= norm; }

    (v, av)
}

// Rayleigh quotient without recomputation
fn rayleigh_quotient_cached(v: &[f32], av: &[f32]) -> f32 {
    let numerator: f32 = v.iter().zip(av.iter()).map(|(vi, avi)| vi * avi).sum();
    let denominator: f32 = v.iter().map(|vi| vi * vi).sum();
    numerator / denominator
}
```

**Impact**: Saves one full matrix-vector product per eigenvector (O(n²) → O(1)).

---

### LOW: Normalized Laplacian Computation

**File**: `src/spectral.rs`
**Lines**: 122-128

**Issue**: Iterates over all n² matrix entries when most are zero.

```rust
// CURRENT: O(n²)
for i in 0..n {
    for j in 0..n {
        laplacian[i * n + j] *= degree_sqrt_inv[i] * degree_sqrt_inv[j];
    }
}
```

**Optimization**: Only normalize non-zero entries (edges + diagonal).

```rust
// OPTIMIZED: O(E)
for &(u, v) in boundary_edges {
    let u = u as usize;
    let v = v as usize;
    if u < n && v < n {
        laplacian[u * n + v] *= degree_sqrt_inv[u] * degree_sqrt_inv[v];
        laplacian[v * n + u] *= degree_sqrt_inv[v] * degree_sqrt_inv[u];
    }
}
for i in 0..n {
    laplacian[i * n + i] *= degree_sqrt_inv[i] * degree_sqrt_inv[i];
}
```

**Impact**: O(n²) → O(E), typically **10-50x faster**.

---

## 2. src/sparse_attention.rs - Sparse Attention Patterns

### HIGH: O(n) Lookup in can_attend

**File**: `src/sparse_attention.rs`
**Line**: 128

**Issue**: Linear search in positions vector.

```rust
pub fn can_attend(&self, query_pos: u16, key_pos: u16) -> bool {
    self.positions.contains(&(query_pos, key_pos))  // ← O(n) linear search
}
```

**Optimization**: Use HashSet or sorted positions with binary search.

```rust
use std::collections::HashSet;

pub struct SparseMask {
    pub positions: Vec<(u16, u16)>,
    position_set: HashSet<(u16, u16)>,  // ← Add HashSet for O(1) lookup
    // ... rest of fields
}

#[inline]
pub fn can_attend(&self, query_pos: u16, key_pos: u16) -> bool {
    self.position_set.contains(&(query_pos, key_pos))  // ← O(1) lookup
}
```

**Alternative** (allocation-free): Keep `positions` sorted and use binary search.

```rust
#[inline]
pub fn can_attend(&self, query_pos: u16, key_pos: u16) -> bool {
    self.positions.binary_search(&(query_pos, key_pos)).is_ok()  // O(log n)
}
```

**Impact**: O(n) → O(1) or O(log n), critical if `can_attend` is called frequently.

---

### CRITICAL: O(n²) Duplicate Detection in build_sparse_positions

**File**: `src/sparse_attention.rs`
**Lines**: 397-424

**Issue**: Using `contains` in nested loops creates O(n²) complexity.

```rust
// Lines 401-404
let pos = (boundary_token, prev_boundary);
if !positions.contains(&pos) {  // ← O(n) search
    positions.push(pos);         // ← Inside loop
}

// Lines 415-419 (similar pattern)
if !positions.contains(&pos) {  // ← O(n) search in nested loop
    positions.push(pos);
}
```

**Expected Complexity**: O(boundary_tokens² × positions.len()) ≈ O(n²) worst case

**Optimization**: Use HashSet for deduplication, then convert to Vec.

```rust
fn build_sparse_positions(
    &self,
    seq_len: usize,
    boundaries: &[u16],
    boundary_tokens: &[u16],
    _target_density: f32,
    _gate: &GatePacket,
) -> Vec<(u16, u16)> {
    use std::collections::HashSet;
    let mut position_set = HashSet::new();  // ← O(1) insert/lookup

    // 1. Intra-partition attention
    if self.config.intra_partition_attention {
        for (partition_idx, &start) in boundaries.iter().enumerate() {
            let end = if partition_idx + 1 < boundaries.len() {
                boundaries[partition_idx + 1] as usize
            } else {
                seq_len
            };

            for i in start as usize..end {
                for j in start as usize..=i {
                    position_set.insert((i as u16, j as u16));  // ← O(1) average
                }
            }
        }
    }

    // 2. Boundary cross-partition attention
    if self.config.boundary_cross_attention {
        for &boundary_token in boundary_tokens {
            for &prev_boundary in boundary_tokens {
                if prev_boundary <= boundary_token {
                    position_set.insert((boundary_token, prev_boundary));
                }
            }

            let window = 4;
            for offset in 0..window {
                let token_pos = boundary_token + offset;
                if (token_pos as usize) < seq_len {
                    for &prev_boundary in boundary_tokens {
                        if prev_boundary <= token_pos {
                            position_set.insert((token_pos, prev_boundary));
                        }
                    }
                }
            }
        }
    }

    position_set.into_iter().collect()
}
```

**Expected Complexity**: O(P + B²) where P = partition positions, B = boundary tokens
**Previous Complexity**: O(P + B² × n) where n = average positions.len()

**Impact**: For seq_len=512, boundary_tokens=20:
- Current: ~20K contains checks ≈ **10M comparisons** worst case
- Optimized: ~20K inserts ≈ **20K operations** (**500x speedup**)

---

### MEDIUM: Inefficient Query Grouping

**File**: `src/sparse_attention.rs`
**Lines**: 235-238

**Issue**: Creates separate Vec for each query position.

```rust
// Group positions by query
let mut positions_by_query: Vec<Vec<u16>> = vec![Vec::new(); seq_len];
for &(query_pos, key_pos) in &mask.positions {
    positions_by_query[query_pos as usize].push(key_pos);
}
```

**Optimization**: Sort positions once, use slice ranges.

```rust
// Sort positions by query: O(m log m) where m = positions.len()
let mut sorted_positions = mask.positions.clone();
sorted_positions.sort_unstable_by_key(|&(q, _)| q);

// Compute attention for each query using binary search for ranges
let mut pos_idx = 0;
for query_pos in 0..seq_len {
    // Find range of positions for this query: O(log m)
    let start = pos_idx;
    while pos_idx < sorted_positions.len() && sorted_positions[pos_idx].0 == query_pos as u16 {
        pos_idx += 1;
    }
    let key_positions = &sorted_positions[start..pos_idx];

    if key_positions.is_empty() {
        continue;
    }

    // ... rest of attention computation
}
```

**Impact**:
- Memory: seq_len allocations eliminated
- Time: O(m log m) sort once vs O(seq_len) allocations + O(m) inserts

---

## 3. src/early_exit.rs - Early Exit Decision Logic

### MEDIUM: Redundant Lambda Stability Calculation

**File**: `src/early_exit.rs`
**Lines**: 305-310, 341-347

**Issue**: Same calculation performed in two places.

```rust
// Line 305-310: In calculate_adaptive_exit_layer
let lambda_delta_abs = gate.lambda_delta().abs() as u32;
let stability = if gate.lambda_prev > 0 {
    let ratio = (lambda_delta_abs * 32768) / gate.lambda_prev.max(1);
    32768u32.saturating_sub(ratio).min(32767) as u16
} else { 0 };

// Line 341-347: In evaluate_exit_conditions (EXACT SAME CODE)
let lambda_delta_abs = gate.lambda_delta().abs() as u32;
let stability = if gate.lambda_prev > 0 {
    let ratio = (lambda_delta_abs * 32768) / gate.lambda_prev.max(1);
    32768u32.saturating_sub(ratio).min(32767) as u16
} else { 0 };
```

**Optimization**: Extract to method, compute once.

```rust
impl GatePacket {
    /// Calculate lambda stability in Q15 format (0-32767)
    /// Higher values = more stable
    #[inline]
    pub fn lambda_stability_q15(&self) -> u16 {
        let lambda_delta_abs = self.lambda_delta().abs() as u32;
        if self.lambda_prev > 0 {
            let ratio = (lambda_delta_abs * 32768) / self.lambda_prev.max(1);
            32768u32.saturating_sub(ratio).min(32767) as u16
        } else {
            0
        }
    }
}

// Usage:
let stability = gate.lambda_stability_q15();
```

**Impact**: Eliminates redundant computation, improves maintainability.

---

### HIGH: O(n log n) Top-K using Full Sort

**File**: `src/early_exit.rs`
**Lines**: 420-428

**Issue**: Sorts entire logits array to find top-k elements.

```rust
fn topk(&self, logits: &[i32], k: usize) -> Vec<usize> {
    if logits.is_empty() || k == 0 {
        return Vec::new();
    }

    let mut indexed: Vec<(usize, i32)> = logits.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| b.1.cmp(&a.1));  // ← O(n log n) for top k elements

    indexed.iter().take(k).map(|(idx, _)| *idx).collect()
}
```

**Expected Complexity**: O(n log n)
**Optimal Complexity**: O(n + k log k)

**Optimization**: Use heap-based selection or partial quickselect.

```rust
use std::collections::BinaryHeap;
use std::cmp::Reverse;

fn topk(&self, logits: &[i32], k: usize) -> Vec<usize> {
    if logits.is_empty() || k == 0 {
        return Vec::new();
    }

    if k >= logits.len() {
        // All elements: O(n log n)
        let mut indexed: Vec<_> = logits.iter().copied().enumerate().collect();
        indexed.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        return indexed.into_iter().map(|(idx, _)| idx).collect();
    }

    // Min-heap of size k: O(n log k)
    let mut heap = BinaryHeap::with_capacity(k);

    for (idx, &val) in logits.iter().enumerate() {
        if heap.len() < k {
            heap.push(Reverse((val, idx)));
        } else if let Some(&Reverse((min_val, _))) = heap.peek() {
            if val > min_val {
                heap.pop();
                heap.push(Reverse((val, idx)));
            }
        }
    }

    heap.into_iter()
        .map(|Reverse((_, idx))| idx)
        .collect()
}
```

**Expected Complexity**: O(n log k) vs O(n log n)

**Impact**: For n=50K vocabulary, k=5:
- Current: O(50K × log(50K)) ≈ **800K operations**
- Optimized: O(50K × log(5)) ≈ **116K operations** (**7x speedup**)

**Alternative** (allocation-free): `select_nth_unstable_by` for O(n) average case:

```rust
fn topk(&self, logits: &[i32], k: usize) -> Vec<usize> {
    let mut indexed: Vec<_> = logits.iter().copied().enumerate().collect();

    if k >= indexed.len() {
        indexed.sort_unstable_by(|a, b| b.1.cmp(&a.1));
    } else {
        // Partition to find k-th largest: O(n) average
        indexed.select_nth_unstable_by(k, |a, b| b.1.cmp(&a.1));
        // Sort only the top k: O(k log k)
        indexed[..k].sort_unstable_by(|a, b| b.1.cmp(&a.1));
    }

    indexed.iter().take(k).map(|(idx, _)| *idx).collect()
}
```

**Complexity**: O(n + k log k) average case.

---

## 4. src/mod_routing.rs - Mixture-of-Depths Routing

### LOW: Mark Boundary Tokens - Minor Optimization

**File**: `src/mod_routing.rs`
**Lines**: 279-287

**Issue**: `step_by` with `stride.max(1)` when `stride` could be 0.

```rust
let stride = routes.len() / boundary_count.max(1);
for i in (0..routes.len()).step_by(stride.max(1)) {  // ← Redundant max(1)
```

**Optimization**: Guard earlier.

```rust
let stride = (routes.len() / boundary_count.max(1)).max(1);
for i in (0..routes.len()).step_by(stride) {
    // ...
}
```

**Impact**: Micro-optimization, eliminates one comparison per iteration.

---

## Summary of Optimizations

| File | Line | Issue | Current | Optimized | Speedup |
|------|------|-------|---------|-----------|---------|
| spectral.rs | 318-326 | Dense matrix-vector | O(n²) | O(E) | **10-200x** |
| spectral.rs | 176-184 | Deflation | O(k×100×n²) | O(50×E) | **100-800x** |
| spectral.rs | 173,177 | Redundant A×v | 2×O(n²) | O(n²) | **2x** |
| spectral.rs | 122-128 | Dense normalization | O(n²) | O(E) | **10-50x** |
| sparse_attention.rs | 128 | Linear lookup | O(n) | O(1) or O(log n) | **n or log n** |
| sparse_attention.rs | 397-424 | Duplicate check | O(n²) | O(n) | **500x** |
| sparse_attention.rs | 235-238 | Query grouping | O(m) allocs | O(m log m) | Memory + cache |
| early_exit.rs | 305,341 | Redundant calc | 2× compute | 1× compute | **2x** |
| early_exit.rs | 420-428 | Full sort for top-k | O(n log n) | O(n log k) | **7x** |

---

## Implementation Priority

### Phase 1: Critical Path (High Impact, Low Risk)
1. ✅ **Sparse matrix representation** (spectral.rs) - **Highest impact**
2. ✅ **HashSet deduplication** (sparse_attention.rs:397-424)
3. ✅ **Heap-based top-k** (early_exit.rs:420-428)

### Phase 2: Performance Enhancements
4. ✅ **Cache A×v in power iteration** (spectral.rs:173,177)
5. ✅ **HashSet for can_attend** (sparse_attention.rs:128)
6. ✅ **Lambda stability method** (early_exit.rs:305,341)

### Phase 3: Advanced Optimizations
7. ✅ **Lanczos algorithm** (spectral.rs:176-184) - Requires more testing
8. ✅ **Sparse normalization** (spectral.rs:122-128)
9. ✅ **Sorted query grouping** (sparse_attention.rs:235-238)

---

## Branch Prediction Analysis

### Good Patterns (Minimal Mispredictions)

1. **early_exit.rs:330-337** - Sequential threshold checks (likely same path)
2. **mod_routing.rs:304-312** - Loop with consistent route type
3. **sparse_attention.rs:243-244** - Early continue on empty (predictable)

### Bad Patterns (High Misprediction Risk)

1. **spectral.rs:85-87** - Random edge bounds check in tight loop
   ```rust
   if u >= n || v >= n {  // ← Unpredictable based on data
       continue;
   }
   ```
   **Fix**: Pre-filter edges or use saturating operations.

2. **sparse_attention.rs:415-419** - `contains` in nested loop
   ```rust
   if !positions.contains(&pos) {  // ← Data-dependent branch
       positions.push(pos);
   }
   ```
   **Fix**: Already addressed by HashSet optimization.

---

## Lookup Table Opportunities

### MEDIUM: Softmax Exp Approximation

**File**: `src/sparse_attention.rs:430-449`

**Current**: Uses `f32::exp()` which is ~100 cycles.

**Optimization**: Lookup table with linear interpolation for exp(-x) in attention range.

```rust
const EXP_TABLE_SIZE: usize = 1024;
static EXP_TABLE: [f32; EXP_TABLE_SIZE] = /* precomputed exp values */;

#[inline]
fn fast_exp(x: f32) -> f32 {
    if x < -10.0 { return 0.0; }
    if x > 0.0 { return x.exp(); }  // Positive values rare in attention

    let idx = (-x * EXP_TABLE_SIZE as f32 / 10.0) as usize;
    if idx >= EXP_TABLE_SIZE - 1 {
        return 0.0;
    }

    // Linear interpolation
    let frac = (-x * EXP_TABLE_SIZE as f32 / 10.0) - idx as f32;
    EXP_TABLE[idx] * (1.0 - frac) + EXP_TABLE[idx + 1] * frac
}
```

**Impact**: 5-10x faster exp, <1% error for attention scores.

---

## Mathematical Simplifications

### spectral.rs: Symmetric Eigenvalue Property

The Laplacian is **symmetric positive semi-definite**, which enables:

1. **Power iteration convergence**: Guaranteed convergence to dominant eigenvector
2. **Real eigenvalues**: No complex arithmetic needed
3. **Orthogonal eigenvectors**: Can use Gram-Schmidt for orthogonalization

**Current code correctly exploits (1) and (2)**, but could use (3) for better numerical stability in deflation.

---

## Recommended Next Steps

1. **Implement Phase 1 optimizations** (sparse matrices, HashSet, heap-based top-k)
2. **Benchmark on realistic workloads** (n=512-2048 tokens, k=8-16 eigenvectors)
3. **Profile with perf/flamegraph** to validate bottlenecks
4. **Consider SIMD** for matrix operations (future work)
5. **Add algorithmic complexity tests** to prevent regressions

---

**Analysis Completed**: 11 optimization opportunities identified
**Estimated Overall Speedup**: 10-50x for eigenvector computation, 5-10x for sparse attention
**Files Analyzed**: 4 core algorithm files, 2,166 lines of code
