# 06 - WebAssembly Integration Analysis

**Agent**: 6 (WASM Integration Specialist)
**Date**: 2026-02-20
**Scope**: ruvector codebase WASM capabilities, build pipeline, SIMD acceleration, memory management, deployment strategies, module loading, and benchmarking framework

---

## Table of Contents

1. [Existing WASM Usage in ruvector](#1-existing-wasm-usage-in-ruvector)
2. [WASM Build Pipeline Compatibility](#2-wasm-build-pipeline-compatibility)
3. [SIMD Acceleration Opportunities](#3-simd-acceleration-opportunities)
4. [Memory Management Patterns](#4-memory-management-patterns)
5. [Browser vs Node.js Deployment Strategies](#5-browser-vs-nodejs-deployment-strategies)
6. [WASM Module Loading and Initialization Patterns](#6-wasm-module-loading-and-initialization-patterns)
7. [Performance Benchmarking Framework for WASM](#7-performance-benchmarking-framework-for-wasm)
8. [Recommendations for the Sublinear-Time Solver](#8-recommendations-for-the-sublinear-time-solver)

---

## 1. Existing WASM Usage in ruvector

### 1.1 Scale of WASM Infrastructure

The ruvector project has a **massive, mature WASM infrastructure**. The workspace defines **27 dedicated WASM crates** in the Cargo workspace, spanning vector database operations, attention mechanisms, graph algorithms, ML inference, and self-learning solvers. This is not an experimental feature -- it is a first-class deployment target.

#### WASM Crate Inventory (27 crates)

| Crate | Description | Target | Size |
|-------|-------------|--------|------|
| `ruvector-wasm` | Core vector DB bindings (HNSW, insert, search, delete) | `wasm32-unknown-unknown` (wasm-bindgen) | ~28 KB src |
| `rvf-solver-wasm` | Self-learning temporal solver (Thompson Sampling, PolicyKernel) | `wasm32-unknown-unknown` (no_std + alloc, `extern "C"`) | ~160 KB compiled |
| `rvf-wasm` | RVF format microkernel for browser/edge vector ops | `wasm32-unknown-unknown` | - |
| `micro-hnsw-wasm` | Neuromorphic HNSW with spiking neural nets | `wasm32-unknown-unknown` | 11.8 KB compiled |
| `ruvector-attention-wasm` | 18+ attention mechanisms (Flash, MoE, Hyperbolic) | `wasm32-unknown-unknown` (wasm-bindgen) | - |
| `ruvector-attention-unified-wasm` | Unified attention API | `wasm32-unknown-unknown` | 339 KB compiled |
| `ruvector-learning-wasm` | MicroLoRA adaptation (<100us latency) | `wasm32-unknown-unknown` | 39 KB compiled |
| `ruvector-nervous-system-wasm` | Bio-inspired neural simulation | `wasm32-unknown-unknown` | 178 KB compiled |
| `ruvector-economy-wasm` | Compute credit management | `wasm32-unknown-unknown` | 181 KB compiled |
| `ruvector-exotic-wasm` | Quantum, hyperbolic, topological | `wasm32-unknown-unknown` | 149 KB compiled |
| `ruvector-sparse-inference-wasm` | Sparse matrix inference with WASM SIMD | `wasm32-unknown-unknown` | - |
| `ruvector-delta-wasm` | Delta operations with SIMD | `wasm32-unknown-unknown` | - |
| `ruvector-mincut-wasm` | Subpolynomial-time dynamic min-cut | `wasm32-unknown-unknown` | - |
| `ruvector-mincut-gated-transformer-wasm` | Gated transformer min-cut | `wasm32-unknown-unknown` | - |
| `ruvector-graph-wasm` | Graph operations | `wasm32-unknown-unknown` | - |
| `ruvector-gnn-wasm` | Graph neural networks | `wasm32-unknown-unknown` | - |
| `ruvector-dag-wasm` | Minimal DAG for browser/embedded | `wasm32-unknown-unknown` | - |
| `ruvector-math-wasm` | Math operations (Wasserstein, manifolds, spherical) | `wasm32-unknown-unknown` | - |
| `ruvector-router-wasm` | Query routing | `wasm32-unknown-unknown` | - |
| `ruvector-fpga-transformer-wasm` | FPGA transformer simulation | `wasm32-unknown-unknown` | - |
| `ruvector-temporal-tensor-wasm` | Temporal tensor operations | `wasm32-unknown-unknown` | - |
| `ruvector-tiny-dancer-wasm` | Lightweight operations | `wasm32-unknown-unknown` | - |
| `ruvector-hyperbolic-hnsw-wasm` | Hyperbolic HNSW | `wasm32-unknown-unknown` | - |
| `ruvector-domain-expansion-wasm` | Cross-domain transfer learning | `wasm32-unknown-unknown` | - |
| `ruvllm-wasm` | LLM inference | `wasm32-unknown-unknown` | - |
| `ruqu-wasm` | Quantum operations | `wasm32-unknown-unknown` | - |
| `exo-wasm` (example) | Exo AI experiment | `wasm32-unknown-unknown` | - |

### 1.2 Two Distinct WASM Binding Strategies

The codebase employs **two fundamentally different WASM integration patterns**:

#### Pattern A: wasm-bindgen + wasm-pack (High-Level, Browser-First)

Used by: `ruvector-wasm`, `ruvector-attention-wasm`, `ruvector-math-wasm`, most `-wasm` crates.

```rust
// crates/ruvector-wasm/src/lib.rs
use wasm_bindgen::prelude::*;
use js_sys::{Float32Array, Object, Promise};
use web_sys::{console, IdbDatabase, IdbFactory};

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
}

#[wasm_bindgen]
pub struct VectorDB { /* ... */ }

#[wasm_bindgen]
impl VectorDB {
    #[wasm_bindgen(constructor)]
    pub fn new(dimensions: usize, metric: Option<String>, use_hnsw: Option<bool>)
        -> Result<VectorDB, JsValue> { /* ... */ }
}
```

Key dependencies: `wasm-bindgen`, `wasm-bindgen-futures`, `js-sys`, `web-sys`, `serde-wasm-bindgen`, `console_error_panic_hook`.

Advantages: Rich JS interop, automatic TypeScript type generation, Promise support, access to Web APIs (IndexedDB, Workers, console).

#### Pattern B: no_std + extern "C" ABI (Low-Level, Minimal)

Used by: `rvf-solver-wasm`, `rvf-wasm`, `micro-hnsw-wasm`.

```rust
// crates/rvf/rvf-solver-wasm/src/lib.rs
#![no_std]
extern crate alloc;

#[no_mangle]
pub extern "C" fn rvf_solver_create() -> i32 {
    registry().create()
}

#[no_mangle]
pub extern "C" fn rvf_solver_train(handle: i32, count: i32, /* ... */) -> i32 { /* ... */ }
```

Key dependencies: `dlmalloc` (global allocator), `libm`, `serde` (no_std + alloc). No wasm-bindgen.

Advantages: Minimal binary size (~160 KB for rvf-solver-wasm, 11.8 KB for micro-hnsw-wasm), no JS runtime dependency, runs on bare wasm32-unknown-unknown, suitable for self-bootstrapping RVF files.

### 1.3 Kernel Pack System (ADR-005)

The `ruvector-wasm` crate includes a sophisticated **Kernel Pack System** (`/crates/ruvector-wasm/src/kernel/`) for secure, sandboxed execution of ML compute kernels via Wasmtime:

- **Manifest parsing** (`manifest.rs`): Declares kernel categories (Positional/RoPE, Normalization/RMSNorm, Activation/SwiGLU, KV-Cache, Adapter/LoRA), tensor specs, resource limits
- **Ed25519 signature verification** (`signature.rs`): Supply chain security for kernel packs
- **SHA256 hash verification** (`hash.rs`): Content integrity
- **Epoch-based execution budgets** (`epoch.rs`): Coarse-grained interruption with configurable tick intervals (10ms server, 1ms embedded)
- **Shared memory protocol** (`memory.rs`): 16-byte aligned allocation, region overlap validation, tensor layout management
- **Kernel runtime** (`runtime.rs`): `KernelRuntime` trait with compile/instantiate/execute lifecycle, mock runtime for testing
- **Trusted allowlist** (`allowlist.rs`): Restricts which kernel IDs may execute

This kernel pack system is directly relevant to the sublinear-time solver because it provides a ready-made infrastructure for sandboxed execution of solver kernels with resource limits.

### 1.4 Self-Bootstrapping WASM (RVF Format)

The `rvf-types` crate defines a `WasmHeader` (`/crates/rvf/rvf-types/src/wasm_bootstrap.rs`) for embedding WASM modules directly inside `.rvf` data files:

```
.rvf file
  +-- WASM_SEG (role=Interpreter, ~50 KB)
  +-- WASM_SEG (role=Microkernel, ~5.5 KB)
  +-- VEC_SEG (data)
```

Roles: `Microkernel`, `Interpreter`, `Combined`, `Extension`, `ControlPlane`.
Targets: `Wasm32`, `WasiP1`, `WasiP2`, `Browser`, `BareTile`.
Feature flags: `WASM_FEAT_SIMD`, `WASM_FEAT_BULK_MEMORY`, `WASM_FEAT_MULTI_VALUE`, `WASM_FEAT_REFERENCE_TYPES`, `WASM_FEAT_THREADS`, `WASM_FEAT_TAIL_CALL`, `WASM_FEAT_GC`, `WASM_FEAT_EXCEPTION_HANDLING`.

### 1.5 Unified WASM TypeScript API

The `@ruvector/wasm-unified` npm package (`/npm/packages/ruvector-wasm-unified/src/index.ts`) provides a high-level TypeScript surface combining all WASM modules:

```typescript
export interface UnifiedEngine {
  attention: AttentionEngine;  // 14+ mechanisms
  learning: LearningEngine;    // MicroLoRA, SONA, BTSP, RL
  nervous: NervousEngine;      // Bio-inspired neural simulation
  economy: EconomyEngine;      // Compute credits
  exotic: ExoticEngine;        // Quantum, hyperbolic, topological
  version(): string;
  getStats(): UnifiedStats;
  init(): Promise<void>;
  dispose(): void;
}
```

---

## 2. WASM Build Pipeline Compatibility

### 2.1 Workspace-Level Configuration

The root `Cargo.toml` defines workspace-level WASM dependencies:

```toml
# /Cargo.toml (workspace)
[workspace.dependencies]
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
js-sys = "0.3"
web-sys = { version = "0.3", features = ["Worker", "MessagePort", "console"] }
getrandom = { version = "0.3", features = ["wasm_js"] }
```

There is also a getrandom compatibility patch for WASM:

```toml
# In ruvector-wasm/Cargo.toml
getrandom02 = { package = "getrandom", version = "0.2", features = ["js"] }
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { workspace = true, features = ["wasm_js"] }
```

And a workspace-level patch for hnsw_rs to use rand 0.8 for WASM compatibility:

```toml
[patch.crates-io]
hnsw_rs = { path = "./patches/hnsw_rs" }
```

### 2.2 Build Profiles

Two distinct WASM build profiles exist:

#### Profile 1: Size-Optimized (for wasm-bindgen crates)

```toml
# crates/ruvector-wasm/Cargo.toml
[profile.release]
opt-level = "z"       # Optimize for size
lto = true            # Link-time optimization
codegen-units = 1     # Single codegen unit
panic = "abort"       # No unwind tables

[profile.release.package."*"]
opt-level = "z"

[package.metadata.wasm-pack.profile.release]
wasm-opt = false      # Disable wasm-opt (already optimized by LTO)
```

#### Profile 2: Size-Optimized + Strip (for no_std crates)

```toml
# crates/rvf/rvf-solver-wasm/Cargo.toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true          # Also strips debug symbols
```

#### Profile 3: Workspace Default Release (native)

```toml
# Root Cargo.toml
[profile.release]
opt-level = 3         # Optimize for speed
lto = "fat"
codegen-units = 1
strip = true
panic = "unwind"      # Keeps unwind tables (unlike WASM profile)
```

### 2.3 Build Tooling

The test script at `/scripts/test/test-wasm.mjs` demonstrates the build command:

```bash
wasm-pack build crates/ruvector-attention-wasm --target web --release
```

For no_std crates like rvf-solver-wasm, the standard cargo command with WASM target is used:

```bash
cargo build --target wasm32-unknown-unknown --release -p rvf-solver-wasm
```

### 2.4 Sublinear-Time Solver Build Compatibility

The rvf-solver-wasm crate provides the closest precedent for a sublinear-time solver WASM build:

- **Target**: `wasm32-unknown-unknown` (no WASI dependency)
- **Allocator**: `dlmalloc` (global allocator for `alloc`)
- **Math**: `libm` (no_std-compatible math functions)
- **Serialization**: `serde` + `serde_json` (no_std + alloc features)
- **Crypto**: `rvf-crypto` (SHAKE-256 witness chain)
- **Panic handler**: `core::arch::wasm32::unreachable()`
- **ABI**: `extern "C"` exports (no wasm-bindgen overhead)
- **Crate type**: `cdylib` only (no rlib)

This approach produces binaries in the ~160 KB range, which is excellent for edge deployment.

---

## 3. SIMD Acceleration Opportunities

### 3.1 Existing WASM SIMD Infrastructure

The codebase has **extensive WASM SIMD128 support** across multiple crates, all using `core::arch::wasm32::*` intrinsics. Every SIMD function provides dual implementations: a `#[cfg(target_feature = "simd128")]` version using WASM SIMD intrinsics and a `#[cfg(not(target_feature = "simd128"))]` scalar fallback.

#### WASM SIMD Operations Already Implemented

| Crate | File | Operations |
|-------|------|------------|
| `ruvector-delta-wasm` | `src/simd.rs` | `f32x4` add, sub, scale, dot, L2 norm, diff, abs, clamp, count_nonzero |
| `ruvector-sparse-inference` | `src/backend/wasm.rs` | `f32x4` dot product, ReLU, vector add, AXPY |
| `ruvector-mincut` | `src/wasm/simd.rs` | `v128` popcount (table lookup method), XOR, boundary computation, batch membership |
| `ruvector-core` | `src/simd_intrinsics.rs` | x86_64 (AVX2, AVX-512, FMA), aarch64 (NEON, unrolled), INT8 quantized, batch operations |

#### SIMD Operations in ruvector-delta-wasm/src/simd.rs (Representative)

```rust
use core::arch::wasm32::*;

#[cfg(target_feature = "simd128")]
pub fn simd_dot(a: &[f32], b: &[f32]) -> f32 {
    let chunks = a.len() / 4;
    let mut sum_vec = f32x4_splat(0.0);
    for i in 0..chunks {
        let offset = i * 4;
        unsafe {
            let a_vec = v128_load(a.as_ptr().add(offset) as *const v128);
            let b_vec = v128_load(b.as_ptr().add(offset) as *const v128);
            let prod = f32x4_mul(a_vec, b_vec);
            sum_vec = f32x4_add(sum_vec, prod);
        }
    }
    // Horizontal sum + remainder handling
    let sum_array: [f32; 4] = unsafe { core::mem::transmute(sum_vec) };
    let mut sum = sum_array[0] + sum_array[1] + sum_array[2] + sum_array[3];
    for i in (chunks * 4)..a.len() { sum += a[i] * b[i]; }
    sum
}
```

#### SIMD Operations in ruvector-sparse-inference/src/backend/wasm.rs (Backend Trait)

```rust
pub struct WasmBackend;

impl Backend for WasmBackend {
    fn dot_product(&self, a: &[f32], b: &[f32]) -> f32 { /* SIMD dispatch */ }
    fn sparse_matmul(&self, matrix: &Array2<f32>, input: &[f32], rows: &[usize]) -> Vec<f32>;
    fn sparse_matmul_accumulate(&self, matrix: &Array2<f32>, input: &[f32], cols: &[usize], output: &mut [f32]);
    fn activation(&self, data: &mut [f32], activation_type: ActivationType); // ReLU via SIMD
    fn add(&self, a: &mut [f32], b: &[f32]);
    fn axpy(&self, a: &mut [f32], b: &[f32], scalar: f32);
    fn name(&self) -> &'static str { "WASM-SIMD" }
    fn simd_width(&self) -> usize { 4 } // 128-bit = 4 x f32
}
```

### 3.2 SIMD Acceleration Opportunities for the Sublinear-Time Solver

Based on the sublinear-time solver's core operations, the following SIMD acceleration points are identified:

| Operation | SIMD Strategy | Expected Speedup | Existing Pattern |
|-----------|---------------|-------------------|------------------|
| Distance computation (dot, cosine, euclidean) | `f32x4_mul` + `f32x4_add` accumulation | 2-4x | `ruvector-delta-wasm/src/simd.rs` |
| Vector normalization | `f32x4_mul` (scale) + `f32x4_add` (L2 norm) | 2-4x | `simd_l2_norm_squared`, `simd_scale` |
| Bitset operations (partition tracking) | `v128_xor`, `v128_and`, popcount via lookup | 4-8x | `ruvector-mincut/src/wasm/simd.rs` |
| Sparse matrix-vector multiply | SIMD dot + sparse row selection | 2-4x | `WasmBackend::sparse_matmul` |
| Activation functions (ReLU, GELU) | `f32x4_max` with zero splat | 2-4x | `relu_wasm_simd` |
| Thompson Sampling bandit updates | Scalar (branching-heavy) | 1x (no benefit) | N/A |
| Sort/selection (top-k) | Scalar (comparison-heavy) | 1x (no benefit) | N/A |

### 3.3 SIMD Feature Detection

The `ruvector-wasm` crate exposes SIMD detection to JS:

```rust
#[wasm_bindgen(js_name = detectSIMD)]
pub fn detect_simd() -> bool {
    #[cfg(target_feature = "simd128")]
    { true }
    #[cfg(not(target_feature = "simd128"))]
    { false }
}
```

For the sublinear-time solver, SIMD should be compiled in via `RUSTFLAGS="-C target-feature=+simd128"` at build time, with scalar fallbacks for environments that do not support it.

### 3.4 Native SIMD Comparison

The native codebase (`ruvector-core/src/simd_intrinsics.rs`) supports:
- **x86_64**: AVX2 (256-bit, 8 x f32), AVX-512 (512-bit, 16 x f32), FMA, INT8 quantized
- **aarch64**: NEON (128-bit, 4 x f32), 4x loop unrolling, FMA via `vfmaq_f32`
- **WASM**: SIMD128 (128-bit, 4 x f32)

WASM SIMD128 provides the same width as NEON (4 x f32) but lacks FMA (`f32x4_fma` is not available in stable WASM SIMD). This means the sublinear-time solver WASM build will be approximately 2-3x slower than a native NEON build for distance computations, and 4-8x slower than an AVX-512 build. However, it will still be significantly faster than scalar fallback.

---

## 4. Memory Management Patterns

### 4.1 Shared Memory Protocol (Kernel Pack System)

The kernel pack system at `/crates/ruvector-wasm/src/kernel/memory.rs` defines a mature shared memory protocol:

```rust
pub struct SharedMemoryProtocol {
    total_size: usize,     // Total memory in bytes
    current_offset: usize, // Bump allocator position
    alignment: usize,      // Typically 16 bytes
}

impl SharedMemoryProtocol {
    pub fn default_settings() -> Self {
        Self::new(256, 16) // 256 pages = 16 MB, 16-byte alignment
    }

    pub fn allocate(&mut self, size: usize) -> Result<usize, KernelError> {
        let aligned_offset = self.align_offset(self.current_offset);
        // ...bounds check...
        self.current_offset = aligned_offset + size;
        Ok(aligned_offset)
    }
}
```

The `KernelInvocationDescriptor` manages tensor memory layout:

```rust
pub struct KernelInvocationDescriptor {
    pub descriptor: KernelDescriptor,  // input_a, input_b, output, scratch, params offsets+sizes
    protocol: SharedMemoryProtocol,
}
```

The `MemoryLayoutValidator` prevents region overlap and bounds violations.

### 4.2 Typed Arrays / Zero-Copy Transfer

The wasm-bindgen crates use `Float32Array` for zero-copy data transfer between JS and WASM:

```rust
// Input: JS Float32Array -> Rust Vec<f32>
pub fn insert(&self, vector: Float32Array, ...) -> Result<String, JsValue> {
    let vector_data: Vec<f32> = vector.to_vec();  // Copy from JS typed array
    // ...
}

// Output: Rust Vec<f32> -> JS Float32Array
pub fn vector(&self) -> Float32Array {
    Float32Array::from(&self.inner.vector[..])  // Copy to JS typed array
}
```

Note: `Float32Array::to_vec()` and `Float32Array::from()` perform copies. True zero-copy requires accessing WASM linear memory directly from JS, which is demonstrated in the pwa-loader:

```javascript
// Zero-copy write into WASM memory
function wasmWrite(data) {
    const ptr = wasmInstance.exports.rvf_alloc(data.length);
    const mem = new Uint8Array(wasmMemory.buffer, ptr, data.length);
    mem.set(data);  // Direct memory write
    return ptr;
}

// Zero-copy read from WASM memory
function wasmRead(ptr, len) {
    return new Uint8Array(wasmMemory.buffer, ptr, len).slice();
}
```

### 4.3 Memory Patterns in rvf-solver-wasm (no_std)

The no_std solver uses `dlmalloc` as global allocator and manages its own instance registry:

```rust
// Global mutable registry - safe in single-threaded WASM
static mut REGISTRY: Registry = Registry::new();
const MAX_INSTANCES: usize = 8;

struct SolverInstance {
    solver: AdaptiveSolver,
    last_result_json: Vec<u8>,   // Heap-allocated via dlmalloc
    policy_json: Vec<u8>,
    witness_chain: Vec<u8>,
}
```

Memory export for external reads uses raw pointer copies:

```rust
#[no_mangle]
pub extern "C" fn rvf_solver_result_read(handle: i32, out_ptr: i32) -> i32 {
    let data = &inst.last_result_json;
    unsafe {
        core::ptr::copy_nonoverlapping(data.as_ptr(), out_ptr as *mut u8, data.len());
    }
    data.len() as i32
}
```

### 4.4 Memory Limits

| Configuration | Max Pages | Memory Limit | Context |
|---------------|-----------|--------------|---------|
| Server runtime | 1024 | 64 MB | `RuntimeConfig::server()` |
| Embedded runtime | 64 | 4 MB | `RuntimeConfig::embedded()` |
| Default shared memory | 256 | 16 MB | `SharedMemoryProtocol::default_settings()` |
| Microkernel (RVF) | 2-4 | 128-256 KB | `WasmHeader` min/max pages |
| WASM page size | 1 | 64 KB | `WASM_PAGE_SIZE = 65536` |

### 4.5 Security Boundary Validation

The `ruvector-wasm` crate enforces input validation at the WASM boundary:

```rust
const MAX_VECTOR_DIMENSIONS: usize = 65536;

#[wasm_bindgen(constructor)]
pub fn new(vector: Float32Array, ...) -> Result<JsVectorEntry, JsValue> {
    let vec_len = vector.length() as usize;
    if vec_len == 0 {
        return Err(JsValue::from_str("Vector cannot be empty"));
    }
    if vec_len > MAX_VECTOR_DIMENSIONS {
        return Err(JsValue::from_str(&format!(
            "Vector dimensions {} exceed maximum allowed {}", vec_len, MAX_VECTOR_DIMENSIONS
        )));
    }
    // ...
}
```

---

## 5. Browser vs Node.js Deployment Strategies

### 5.1 Browser Deployment (Primary)

The ruvector-wasm crate is browser-first, using:

- **IndexedDB persistence**: `web-sys` features include `IdbDatabase`, `IdbFactory`, `IdbObjectStore`, `IdbRequest`, `IdbTransaction`, `IdbOpenDbRequest` (`/crates/ruvector-wasm/Cargo.toml`)
- **Web Workers**: Embedded JavaScript worker pool (`/crates/ruvector-wasm/src/worker-pool.js`, `/crates/ruvector-wasm/src/worker.js`) for parallel operations
- **Tracing via console**: `tracing-wasm` sends logs to browser dev tools
- **Promise-based async**: `wasm-bindgen-futures` for async operations
- **getrandom via JS**: `getrandom` with `wasm_js` feature uses `crypto.getRandomValues()`
- **PWA support**: The pwa-loader example (`/examples/pwa-loader/app.js`) demonstrates offline-capable WASM loading

#### Browser Loading Pattern

```javascript
// From examples/pwa-loader/app.js
async function loadWasm() {
    const response = await fetch(WASM_PATH);
    const bytes = await response.arrayBuffer();
    const importObject = { env: {} };
    const result = await WebAssembly.instantiate(bytes, importObject);
    wasmInstance = result.instance;
    wasmMemory = wasmInstance.exports.memory;
}
```

#### Browser SIMD Support

WASM SIMD128 is supported in Chrome 91+, Firefox 89+, Safari 16.4+, and Edge 91+. This covers >95% of active browsers as of 2026. Feature detection can be done via:

```javascript
const simdSupported = WebAssembly.validate(
    new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,0,1,123,3,2,1,0,10,10,1,8,0,65,0,253,15,253,98,11])
);
```

### 5.2 Node.js Deployment

The project supports Node.js via:

- **wasm-pack `--target nodejs`**: Generates CommonJS bindings
- **Direct instantiation** from test scripts (`/scripts/test/test-wasm.mjs`):

```javascript
import { readFileSync } from 'fs';
const wasmBuffer = readFileSync(wasmPath);
const mathWasm = await import(join(pkgPath, 'ruvector_math_wasm.js'));
await mathWasm.default(wasmBuffer);
```

- **Edge-net example**: `/examples/edge-net/pkg/node/` provides Node-specific WASM packages

Node.js has had WASM SIMD support since v16.4 (V8 9.1+). For the sublinear-time solver, Node.js deployment enables server-side and CLI usage with the same WASM binary.

### 5.3 Edge / Embedded Deployment

The `micro-hnsw-wasm` crate (11.8 KB) and `rvf-solver-wasm` (~160 KB) demonstrate ultra-compact deployment:

- **iOS/Swift**: `/examples/wasm/ios/` includes Swift resources with embedded WASM
- **Self-bootstrapping**: The WASM_SEG system embeds WASM interpreters inside data files
- **Target platforms**: `WasmTarget::Wasm32`, `WasiP1`, `WasiP2`, `Browser`, `BareTile`

### 5.4 Deployment Target Matrix

| Target | WASM Format | Binding | SIMD | Size Budget | Persistence |
|--------|-------------|---------|------|-------------|-------------|
| Browser (Chrome/FF/Safari) | wasm-bindgen | JS glue + TS types | SIMD128 | <500 KB | IndexedDB |
| Node.js (>= 16.4) | wasm-bindgen (nodejs) or raw | CommonJS/ESM | SIMD128 | <1 MB | fs |
| Cloudflare Workers | wasm-bindgen (web) | ESM | SIMD128 | <1 MB | KV |
| iOS/Swift | raw wasm32 | C FFI | Optional | <200 KB | CoreData |
| Bare-metal / RVF | no_std cdylib | extern "C" | Optional | <200 KB | None |

---

## 6. WASM Module Loading and Initialization Patterns

### 6.1 Pattern 1: wasm-bindgen Auto-Init

Used by most WASM crates. The `#[wasm_bindgen(start)]` attribute runs initialization automatically:

```rust
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
}
```

JS side (generated by wasm-pack):

```javascript
import init, { VectorDB } from './ruvector_wasm.js';
await init();  // Loads + instantiates + runs start function
const db = new VectorDB(384, 'cosine', true);
```

### 6.2 Pattern 2: Manual WebAssembly.instantiate

Used by the pwa-loader and no_std modules:

```javascript
const response = await fetch(WASM_PATH);
const bytes = await response.arrayBuffer();
const importObject = { env: {} };
const result = await WebAssembly.instantiate(bytes, importObject);
wasmInstance = result.instance;
wasmMemory = wasmInstance.exports.memory;
```

This pattern offers maximum control: the host can inspect exports before calling any function, handle errors granularly, and manage memory directly.

### 6.3 Pattern 3: Streaming Instantiation

For large modules, `WebAssembly.instantiateStreaming` should be used (not currently in the codebase but recommended):

```javascript
const result = await WebAssembly.instantiateStreaming(
    fetch(WASM_PATH),
    importObject
);
```

This starts compiling while bytes are still downloading, reducing load time by up to 50%.

### 6.4 Pattern 4: Unified Engine Lazy Init

The `@ruvector/wasm-unified` uses lazy initialization:

```typescript
let defaultEngine: UnifiedEngine | null = null;

export async function getDefaultEngine(): Promise<UnifiedEngine> {
    if (!defaultEngine) {
        defaultEngine = await createUnifiedEngine();
        await defaultEngine.init();
    }
    return defaultEngine;
}
```

### 6.5 Pattern 5: Instance Registry (rvf-solver-wasm)

The solver WASM uses a handle-based instance registry:

```rust
static mut REGISTRY: Registry = Registry::new();  // Max 8 concurrent solvers

// JS creates solver:
let handle = wasmInstance.exports.rvf_solver_create();
// JS uses solver:
wasmInstance.exports.rvf_solver_train(handle, 100, 1, 10, seedLo, seedHi);
// JS reads result:
let len = wasmInstance.exports.rvf_solver_result_len(handle);
let ptr = wasmInstance.exports.rvf_solver_alloc(len);
wasmInstance.exports.rvf_solver_result_read(handle, ptr);
let json = new TextDecoder().decode(new Uint8Array(wasmMemory.buffer, ptr, len));
// JS destroys:
wasmInstance.exports.rvf_solver_destroy(handle);
```

This is the recommended pattern for the sublinear-time solver because it:
- Supports multiple concurrent solver instances
- Avoids global state issues
- Enables resource cleanup
- Works across all deployment targets (browser, Node, bare-metal)

---

## 7. Performance Benchmarking Framework for WASM

### 7.1 Existing Benchmark Infrastructure

#### In-WASM Benchmark Function

The `ruvector-wasm` crate includes a built-in benchmark export:

```rust
#[wasm_bindgen(js_name = benchmark)]
pub fn benchmark(name: &str, iterations: usize, dimensions: usize) -> Result<f64, JsValue> {
    let start = Instant::now();
    for i in 0..iterations {
        let vector: Vec<f32> = (0..dimensions)
            .map(|_| js_sys::Math::random() as f32)
            .collect();
        let vector_arr = Float32Array::from(&vector[..]);
        db.insert(vector_arr, Some(format!("vec_{}", i)), None)?;
    }
    let duration = start.elapsed();
    Ok(iterations as f64 / duration.as_secs_f64())
}
```

#### WASM Solver Benchmark Binary

The `/examples/benchmarks/src/bin/wasm_solver_bench.rs` provides a native vs WASM comparison framework:

```
WASM vs Native AGI Solver Benchmark
  Config: holdout=50, training=50, cycles=3, budget=200

  NATIVE SOLVER RESULTS
  Mode          Acc%       Cost    Noise%    Time     Pass
  A baseline   xx.x%     xxx.x    xx.x%    xxxms    PASS
  B compiler   xx.x%     xxx.x    xx.x%    xxxms    PASS
  C learned    xx.x%     xxx.x    xx.x%    xxxms    PASS

  WASM REFERENCE METRICS
  Native total time:  xxxms
  WASM expected:      ~xxxms (2-5x native)
```

This establishes the expected WASM overhead: **2-5x slower than native** for the self-learning solver workload.

#### SIMD Benchmarks

The `/crates/prime-radiant/benches/simd_benchmarks.rs` and `/crates/ruvector-sparse-inference/benches/simd_kernels.rs` provide Criterion benchmarks for SIMD operations that can be adapted for WASM SIMD.

### 7.2 Recommended Benchmarking Framework for the Sublinear-Time Solver

```
sublinear-time-solver/benches/
  wasm_bench.rs          -- In-Rust Criterion benchmarks (native baseline)
  wasm_bench.mjs         -- Node.js WASM performance runner
  wasm_bench.html        -- Browser WASM performance runner
  bench_harness.rs       -- Shared benchmark harness (puzzle generation)
```

#### Metrics to Track

| Metric | Description | Measurement |
|--------|-------------|-------------|
| `solve_throughput` | Puzzles solved per second | `iterations / elapsed_secs` |
| `solve_latency_p50` | Median solve time | Percentile of individual solve times |
| `solve_latency_p99` | 99th percentile solve time | Percentile of individual solve times |
| `memory_peak_bytes` | Peak WASM linear memory usage | `memory.buffer.byteLength` |
| `module_load_ms` | Time to instantiate WASM module | `performance.now()` around `WebAssembly.instantiate` |
| `simd_speedup` | SIMD vs scalar performance ratio | Compare SIMD build vs non-SIMD build |
| `wasm_native_ratio` | WASM-to-native performance overhead | Compare WASM throughput vs native Criterion results |
| `binary_size_bytes` | Compiled .wasm file size | `wc -c *.wasm` |
| `accuracy_parity` | Solver accuracy matches native | Bit-exact or epsilon comparison of results |

#### Benchmark Protocol

1. **Native baseline**: Run the solver natively with Criterion (3+ iterations, warm-up)
2. **WASM baseline**: Load the same solver as WASM, run identical workload in Node.js
3. **WASM SIMD**: Build with `RUSTFLAGS="-C target-feature=+simd128"`, measure speedup
4. **Browser measurement**: Run in Chrome with `performance.now()`, measure real-world latency
5. **Size budget**: Track .wasm binary size across commits (regression alerts if >200 KB)
6. **Accuracy validation**: Compare solver output JSON between native and WASM (must match to f64 epsilon)

---

## 8. Recommendations for the Sublinear-Time Solver

### 8.1 Binding Strategy: Use no_std + extern "C" (Pattern B)

For the sublinear-time solver WASM module, adopt the `rvf-solver-wasm` pattern:

- **no_std + alloc**: Minimizes binary size, avoids JS runtime dependency
- **dlmalloc global allocator**: Proven in rvf-solver-wasm
- **extern "C" exports**: Maximum portability (browser, Node, embedded, bare-metal)
- **Handle-based instance registry**: Supports concurrent solver instances
- **Result reads via pointer+length**: JSON serialization of results into WASM memory, host reads via typed array view

Do not use wasm-bindgen for the core solver. A thin wasm-bindgen wrapper can be created separately if a richer JS API is needed.

### 8.2 SIMD Strategy: Conditional Compilation

```rust
// In the solver crate
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod simd_wasm {
    use core::arch::wasm32::*;
    pub fn distance_l2_simd(a: &[f32], b: &[f32]) -> f32 { /* SIMD128 */ }
}

#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
mod simd_wasm {
    pub fn distance_l2_simd(a: &[f32], b: &[f32]) -> f32 { /* scalar fallback */ }
}
```

Build two variants:
- `solver.wasm` -- scalar fallback (maximum compatibility)
- `solver-simd.wasm` -- SIMD128 enabled (Chrome 91+, FF 89+, Safari 16.4+, Node 16.4+)

### 8.3 Memory Strategy: Bump Allocator + Shared Memory Protocol

Adopt the `SharedMemoryProtocol` pattern from the kernel pack system:

1. Allocate a fixed arena at solver creation (e.g., 256 pages = 16 MB)
2. Use 16-byte aligned bump allocation for tensor data
3. Reset the allocator between solve invocations (amortized O(1))
4. Validate memory regions before kernel execution
5. Export `memory` so the host can directly view/write typed arrays without copying

### 8.4 Build Profile

```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

Target binary size: <200 KB (consistent with existing rvf-solver-wasm at ~160 KB).

### 8.5 Feature Detection Export

```rust
#[no_mangle]
pub extern "C" fn solver_capabilities() -> u32 {
    let mut caps = 0u32;
    #[cfg(target_feature = "simd128")]
    { caps |= 0x01; }  // SIMD available
    #[cfg(feature = "thompson-sampling")]
    { caps |= 0x02; }  // Thompson Sampling enabled
    #[cfg(feature = "witness-chain")]
    { caps |= 0x04; }  // Witness chain enabled
    caps
}
```

### 8.6 Testing Strategy

- Use `wasm-bindgen-test` with `run_in_browser` for browser tests (existing pattern)
- Use the Node.js test harness at `/scripts/test/test-wasm.mjs` as a template
- Validate accuracy parity with native build via `wasm_solver_bench`
- Run SIMD-specific tests with `RUSTFLAGS="-C target-feature=+simd128"` in CI

---

## Appendix A: File Reference

### Core WASM Source Files

| File | Purpose |
|------|---------|
| `/crates/ruvector-wasm/src/lib.rs` | Main VectorDB WASM bindings (wasm-bindgen) |
| `/crates/ruvector-wasm/src/kernel/mod.rs` | Kernel pack system entry point |
| `/crates/ruvector-wasm/src/kernel/memory.rs` | Shared memory protocol, bump allocator |
| `/crates/ruvector-wasm/src/kernel/runtime.rs` | Kernel runtime trait, mock runtime, manager |
| `/crates/ruvector-wasm/src/kernel/epoch.rs` | Epoch-based execution budgets |
| `/crates/ruvector-wasm/src/kernel/signature.rs` | Ed25519 kernel pack verification |
| `/crates/ruvector-wasm/src/kernel/manifest.rs` | Kernel manifest parsing |
| `/crates/ruvector-wasm/Cargo.toml` | WASM dependency configuration |

### SIMD Source Files

| File | Purpose |
|------|---------|
| `/crates/ruvector-delta-wasm/src/simd.rs` | WASM SIMD128 f32x4 operations |
| `/crates/ruvector-sparse-inference/src/backend/wasm.rs` | WASM SIMD backend with Backend trait |
| `/crates/ruvector-mincut/src/wasm/simd.rs` | WASM SIMD128 bitset operations |
| `/crates/ruvector-core/src/simd_intrinsics.rs` | Native SIMD (AVX2/AVX-512/NEON) reference |

### Solver WASM Source Files

| File | Purpose |
|------|---------|
| `/crates/rvf/rvf-solver-wasm/src/lib.rs` | Self-learning solver WASM exports (no_std) |
| `/crates/rvf/rvf-solver-wasm/src/engine.rs` | Adaptive solver engine |
| `/crates/rvf/rvf-solver-wasm/src/policy.rs` | PolicyKernel with Thompson Sampling |
| `/crates/rvf/rvf-solver-wasm/Cargo.toml` | no_std WASM build configuration |

### Build and Test Files

| File | Purpose |
|------|---------|
| `/Cargo.toml` | Workspace WASM dependencies and build profiles |
| `/scripts/test/test-wasm.mjs` | Node.js WASM test runner |
| `/examples/benchmarks/src/bin/wasm_solver_bench.rs` | Native vs WASM benchmark comparison |
| `/examples/pwa-loader/app.js` | Browser WASM loading and memory management |

### RVF Self-Bootstrap Files

| File | Purpose |
|------|---------|
| `/crates/rvf/rvf-types/src/wasm_bootstrap.rs` | WasmHeader, WasmRole, WasmTarget, feature flags |

### TypeScript/npm Files

| File | Purpose |
|------|---------|
| `/npm/packages/ruvector-wasm-unified/src/index.ts` | Unified WASM engine TypeScript API |

---

## Appendix B: WASM Binary Size Inventory

| Binary | Size | Strategy |
|--------|------|----------|
| `micro_hnsw.wasm` | 11.8 KB | no_std, bare minimum |
| `ruvector_learning_wasm_bg.wasm` | 39 KB | wasm-bindgen |
| `ruvector_exotic_wasm_bg.wasm` | 149 KB | wasm-bindgen |
| `ruvector_nervous_system_wasm_bg.wasm` | 178 KB | wasm-bindgen |
| `ruvector_economy_wasm_bg.wasm` | 181 KB | wasm-bindgen |
| `ruvector_attention_unified_wasm_bg.wasm` | 339 KB | wasm-bindgen |
| `rvf-solver-wasm` (estimated) | ~160 KB | no_std + dlmalloc |

The sublinear-time solver should target the **<200 KB** range using the no_std approach, consistent with `rvf-solver-wasm`.
