# ADR-STS-004: WASM and Cross-Platform Compilation Strategy

**Status**: Accepted
**Date**: 2026-02-20
**Authors**: RuVector Architecture Team
**Deciders**: Architecture Review Board

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-20 | RuVector Team | Initial proposal |
| 1.0 | 2026-02-20 | RuVector Team | Accepted: full implementation complete |

---

## Context

### Multi-Platform Deployment Requirement

RuVector deploys across four target platforms with distinct constraints:

| Platform | ISA | SIMD | Threads | Memory | Target Triple |
|----------|-----|------|---------|--------|--------------|
| Server (Linux/macOS) | x86_64 | AVX-512/AVX2/SSE4.1 | Full (Rayon) | 2+ GB | x86_64-unknown-linux-gnu |
| Edge (Apple Silicon) | ARM64 | NEON | Full (Rayon) | 512 MB | aarch64-apple-darwin |
| Browser | wasm32 | SIMD128 | Web Workers | 4-8 MB | wasm32-unknown-unknown |
| Cloudflare Workers | wasm32 | None | Single | 128 MB | wasm32-unknown-unknown |
| Node.js (NAPI) | Native | Native | Full | 512 MB | via napi-rs |

### Existing WASM Infrastructure

RuVector has 15+ WASM crates following the **Core-Binding-Surface** pattern:

```
ruvector-core       →  ruvector-wasm         →  @ruvector/core (npm)
ruvector-graph      →  ruvector-graph-wasm   →  @ruvector/graph (npm)
ruvector-attention   →  ruvector-attention-wasm →  @ruvector/attention (npm)
ruvector-gnn        →  ruvector-gnn-wasm     →  @ruvector/gnn (npm)
ruvector-math       →  ruvector-math-wasm    →  @ruvector/math (npm)
```

Each WASM crate uses `wasm-bindgen 0.2`, `serde-wasm-bindgen`, `js-sys 0.3`, and `getrandom 0.3` with `wasm_js` feature.

### WASM Constraints for Solver

- No `std::thread` — all parallelism via Web Workers
- No `std::fs` / `std::net` — no persistent storage, no network
- Default linear memory: 16 MB (expandable to ~4 GB)
- `parking_lot` required instead of `std::sync::Mutex`
- `getrandom/wasm_js` for randomness (Hybrid Random Walk, Monte Carlo)
- No dynamic linking — all code in single module

### Performance Targets

| Platform | 10K solve | 100K solve | Memory Budget |
|----------|-----------|------------|---------------|
| Server (AVX2) | < 2 ms | < 50 ms | 2 GB |
| Edge (NEON) | < 5 ms | < 100 ms | 512 MB |
| Browser (SIMD128) | < 50 ms | < 500 ms | 8 MB |
| Edge (Cloudflare) | < 10 ms | < 200 ms | 128 MB |
| Node.js (NAPI) | < 3 ms | < 60 ms | 512 MB |

---

## Decision

### 1. Three-Crate Pattern

Follow established RuVector convention with three crates:

```
crates/ruvector-solver/          # Core Rust (no platform deps)
crates/ruvector-solver-wasm/     # wasm-bindgen bindings
crates/ruvector-solver-node/     # NAPI-RS bindings
```

#### Cargo.toml for ruvector-solver (core):

```toml
[package]
name = "ruvector-solver"
version = "0.1.0"
edition = "2021"
rust-version = "1.77"

[features]
default = []
nalgebra-backend = ["nalgebra"]
ndarray-backend = ["ndarray"]
parallel = ["rayon", "crossbeam"]
simd = []
wasm = []
full = ["nalgebra-backend", "ndarray-backend", "parallel"]

# Algorithm features
neumann = []
forward-push = []
backward-push = []
hybrid-random-walk = ["getrandom"]
true-solver = ["neumann"]  # TRUE uses Neumann internally
cg = []
bmssp = []
all-algorithms = ["neumann", "forward-push", "backward-push",
                  "hybrid-random-walk", "true-solver", "cg", "bmssp"]

[dependencies]
serde = { workspace = true, features = ["derive"] }
nalgebra = { workspace = true, optional = true, default-features = false }
ndarray = { workspace = true, optional = true }
rayon = { workspace = true, optional = true }
crossbeam = { workspace = true, optional = true }
getrandom = { workspace = true, optional = true }

[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { workspace = true, features = ["wasm_js"] }
```

#### Cargo.toml for ruvector-solver-wasm:

```toml
[package]
name = "ruvector-solver-wasm"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
ruvector-solver = { path = "../ruvector-solver", default-features = false,
    features = ["wasm", "neumann", "forward-push", "backward-push", "cg"] }
wasm-bindgen = { workspace = true }
serde-wasm-bindgen = "0.6"
js-sys = { workspace = true }
web-sys = { workspace = true, features = ["console"] }
getrandom = { workspace = true, features = ["wasm_js"] }

[profile.release]
opt-level = "s"   # Optimize for size in WASM
lto = true
```

#### Cargo.toml for ruvector-solver-node:

```toml
[package]
name = "ruvector-solver-node"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
ruvector-solver = { path = "../ruvector-solver",
    features = ["full", "all-algorithms"] }
napi = { workspace = true, features = ["async"] }
napi-derive = { workspace = true }
tokio = { workspace = true, features = ["rt-multi-thread"] }
```

### 2. SIMD Strategy Per Platform

#### Architecture Detection and Dispatch

```rust
/// SIMD dispatcher for solver hot paths
pub mod simd {
    #[cfg(target_arch = "x86_64")]
    pub fn spmv_simd(vals: &[f32], cols: &[u32], x: &[f32]) -> f32 {
        if is_x86_feature_detected!("avx512f") {
            unsafe { spmv_avx512(vals, cols, x) }
        } else if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            unsafe { spmv_avx2_fma(vals, cols, x) }
        } else {
            spmv_scalar(vals, cols, x)
        }
    }

    #[cfg(target_arch = "aarch64")]
    pub fn spmv_simd(vals: &[f32], cols: &[u32], x: &[f32]) -> f32 {
        unsafe { spmv_neon_unrolled(vals, cols, x) }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn spmv_simd(vals: &[f32], cols: &[u32], x: &[f32]) -> f32 {
        // WASM SIMD128 via core::arch::wasm32
        #[cfg(target_feature = "simd128")]
        {
            unsafe { spmv_wasm_simd128(vals, cols, x) }
        }
        #[cfg(not(target_feature = "simd128"))]
        {
            spmv_scalar(vals, cols, x)
        }
    }

    /// AVX2+FMA SpMV accumulation with 4x unrolling
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2,fma")]
    unsafe fn spmv_avx2_fma(vals: &[f32], cols: &[u32], x: &[f32]) -> f32 {
        use std::arch::x86_64::*;
        let mut acc0 = _mm256_setzero_ps();
        let mut acc1 = _mm256_setzero_ps();
        let n = vals.len();
        let chunks = n / 16;

        for i in 0..chunks {
            let base = i * 16;
            // Gather x values using column indices
            let idx0 = _mm256_loadu_si256(cols.as_ptr().add(base) as *const __m256i);
            let idx1 = _mm256_loadu_si256(cols.as_ptr().add(base + 8) as *const __m256i);
            let x0 = _mm256_i32gather_ps::<4>(x.as_ptr(), idx0);
            let x1 = _mm256_i32gather_ps::<4>(x.as_ptr(), idx1);
            let v0 = _mm256_loadu_ps(vals.as_ptr().add(base));
            let v1 = _mm256_loadu_ps(vals.as_ptr().add(base + 8));
            acc0 = _mm256_fmadd_ps(v0, x0, acc0);
            acc1 = _mm256_fmadd_ps(v1, x1, acc1);
        }

        // Horizontal sum
        let sum = _mm256_add_ps(acc0, acc1);
        let hi = _mm256_extractf128_ps::<1>(sum);
        let lo = _mm256_castps256_ps128(sum);
        let sum128 = _mm_add_ps(hi, lo);
        let shuf = _mm_movehdup_ps(sum128);
        let sums = _mm_add_ps(sum128, shuf);
        let shuf2 = _mm_movehl_ps(sums, sums);
        let result = _mm_add_ss(sums, shuf2);

        let mut total = _mm_cvtss_f32(result);

        // Scalar remainder
        for j in (chunks * 16)..n {
            total += vals[j] * x[cols[j] as usize];
        }
        total
    }

    /// NEON SpMV with 4x unrolling for ARM64
    #[cfg(target_arch = "aarch64")]
    unsafe fn spmv_neon_unrolled(vals: &[f32], cols: &[u32], x: &[f32]) -> f32 {
        use std::arch::aarch64::*;
        let mut acc0 = vdupq_n_f32(0.0);
        let mut acc1 = vdupq_n_f32(0.0);
        let mut acc2 = vdupq_n_f32(0.0);
        let mut acc3 = vdupq_n_f32(0.0);
        let n = vals.len();
        let chunks = n / 16;

        for i in 0..chunks {
            let base = i * 16;
            // Manual gather for NEON (no hardware gather instruction)
            let mut xbuf = [0.0f32; 16];
            for k in 0..16 {
                xbuf[k] = *x.get_unchecked(cols[base + k] as usize);
            }
            let v0 = vld1q_f32(vals.as_ptr().add(base));
            let v1 = vld1q_f32(vals.as_ptr().add(base + 4));
            let v2 = vld1q_f32(vals.as_ptr().add(base + 8));
            let v3 = vld1q_f32(vals.as_ptr().add(base + 12));
            let x0 = vld1q_f32(xbuf.as_ptr());
            let x1 = vld1q_f32(xbuf.as_ptr().add(4));
            let x2 = vld1q_f32(xbuf.as_ptr().add(8));
            let x3 = vld1q_f32(xbuf.as_ptr().add(12));
            acc0 = vfmaq_f32(acc0, v0, x0);
            acc1 = vfmaq_f32(acc1, v1, x1);
            acc2 = vfmaq_f32(acc2, v2, x2);
            acc3 = vfmaq_f32(acc3, v3, x3);
        }

        let sum01 = vaddq_f32(acc0, acc1);
        let sum23 = vaddq_f32(acc2, acc3);
        let sum = vaddq_f32(sum01, sum23);
        let mut total = vaddvq_f32(sum);

        for j in (chunks * 16)..n {
            total += vals[j] * x[cols[j] as usize];
        }
        total
    }
}
```

### 3. Conditional Compilation Architecture

```rust
// Parallelism: Rayon on native, single-threaded on WASM
#[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
fn batch_solve_parallel(problems: &[SparseSystem]) -> Vec<SolverResult> {
    use rayon::prelude::*;
    problems.par_iter().map(|p| solve_single(p)).collect()
}

#[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
fn batch_solve_parallel(problems: &[SparseSystem]) -> Vec<SolverResult> {
    problems.iter().map(|p| solve_single(p)).collect()
}

// Random number generation
#[cfg(not(target_arch = "wasm32"))]
fn random_seed() -> u64 {
    use std::time::SystemTime;
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
        .unwrap().as_nanos() as u64
}

#[cfg(target_arch = "wasm32")]
fn random_seed() -> u64 {
    let mut buf = [0u8; 8];
    getrandom::getrandom(&mut buf).expect("getrandom failed");
    u64::from_le_bytes(buf)
}
```

### 4. WASM-Specific Patterns

#### Web Worker Pool (JavaScript side):

```javascript
// Following existing ruvector-wasm/src/worker-pool.js pattern
class SolverWorkerPool {
    constructor(numWorkers = navigator.hardwareConcurrency || 4) {
        this.workers = [];
        this.queue = [];
        for (let i = 0; i < numWorkers; i++) {
            const worker = new Worker(new URL('./solver-worker.js', import.meta.url));
            worker.onmessage = (e) => this._onResult(i, e.data);
            this.workers.push({ worker, busy: false });
        }
    }

    async solve(config) {
        return new Promise((resolve, reject) => {
            const free = this.workers.find(w => !w.busy);
            if (free) {
                free.busy = true;
                free.worker.postMessage({
                    type: 'solve',
                    config,
                    // Transfer ArrayBuffer for zero-copy
                    matrix: config.matrix
                }, [config.matrix.buffer]);
                free.resolve = resolve;
                free.reject = reject;
            } else {
                this.queue.push({ config, resolve, reject });
            }
        });
    }
}
```

#### SharedArrayBuffer (when COOP/COEP available):

```javascript
// Check for cross-origin isolation
if (typeof SharedArrayBuffer !== 'undefined') {
    // Zero-copy shared matrix between main thread and workers
    const shared = new SharedArrayBuffer(matrix.byteLength);
    new Float32Array(shared).set(matrix);
    // Workers can read directly without transfer
    workers.forEach(w => w.postMessage({ type: 'set_matrix', buffer: shared }));
}
```

#### IndexedDB for Persistence:

```javascript
// Cache solver preprocessing results (TRUE sparsifier, etc.)
class SolverCache {
    async store(key, sparsifier) {
        const db = await this._openDB();
        const tx = db.transaction('cache', 'readwrite');
        await tx.objectStore('cache').put({
            key,
            data: sparsifier.buffer,
            timestamp: Date.now()
        });
    }

    async load(key) {
        const db = await this._openDB();
        const tx = db.transaction('cache', 'readonly');
        return tx.objectStore('cache').get(key);
    }
}
```

### 5. Build Pipeline

```bash
# WASM build (production)
cd crates/ruvector-solver-wasm
wasm-pack build --target web --release
wasm-opt -O3 -o pkg/ruvector_solver_wasm_bg_opt.wasm pkg/ruvector_solver_wasm_bg.wasm
mv pkg/ruvector_solver_wasm_bg_opt.wasm pkg/ruvector_solver_wasm_bg.wasm

# WASM build with SIMD128
RUSTFLAGS="-C target-feature=+simd128" wasm-pack build --target web --release

# Node.js build
cd crates/ruvector-solver-node
npm run build  # napi build --release

# Multi-platform CI
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target aarch64-apple-darwin
cargo build --release --target wasm32-unknown-unknown
```

### 6. WASM Bundle Size Budget

| Component | Estimated Size (gzipped) | Budget |
|-----------|-------------------------|--------|
| Solver core (CG + Neumann + Push) | ~80 KB | 100 KB |
| SIMD128 kernels | ~15 KB | 20 KB |
| wasm-bindgen glue | ~10 KB | 15 KB |
| serde-wasm-bindgen | ~20 KB | 25 KB |
| **Total** | **~125 KB** | **160 KB** |

Optimization: Use `opt-level = "s"` and `wasm-opt -Oz` for size-constrained deployments.

---

## Consequences

### Positive

1. **Universal deployment**: Same solver logic runs on all 5 platforms
2. **Platform-optimized**: Each target gets architecture-specific SIMD kernels
3. **Minimal overhead**: WASM binary < 160 KB gzipped
4. **Web Worker parallelism**: Browser gets multi-threaded solver via worker pool
5. **SharedArrayBuffer**: Zero-copy where cross-origin isolation available
6. **Proven pattern**: Follows RuVector's established Core-Binding-Surface architecture

### Negative

1. **WASM algorithm subset**: TRUE and BMSSP excluded from browser target (preprocessing cost)
2. **SIMD gap**: WASM SIMD128 is 2-4x slower than AVX2 for equivalent operations
3. **No WASM threads**: Web Workers add message-passing overhead vs native threads
4. **Gather limitation**: NEON and WASM lack hardware gather; manual gather adds latency

### Neutral

1. nalgebra compiles to WASM with `default-features = false` — no code changes needed
2. WASM SIMD128 support is universal in modern browsers (Chrome 91+, Firefox 89+, Safari 16.4+)

---

## Implementation Status

WASM bindings complete via wasm-bindgen in ruvector-solver-wasm crate. All 7 algorithms exposed to JavaScript. TypedArray zero-copy for matrix data. Feature-gated compilation (wasm feature). Scalar SpMV fallback when SIMD unavailable. 32-bit index support for wasm32 memory model.

---

## References

- [06-wasm-integration.md](../06-wasm-integration.md) — Detailed WASM analysis
- [08-performance-analysis.md](../08-performance-analysis.md) — Platform performance targets
- [11-typescript-integration.md](../11-typescript-integration.md) — TypeScript type generation
- ADR-005 — RuVector WASM runtime integration
