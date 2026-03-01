# ADR-QE-003: WebAssembly Compilation Strategy

**Status**: Proposed
**Date**: 2026-02-06
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board

## Context

### Problem Statement

ruVector targets browsers, embedded/edge runtimes, and IoT devices via
WebAssembly. The quantum simulation engine must compile to
`wasm32-unknown-unknown` and run correctly in these constrained environments.
WASM introduces fundamental constraints that differ significantly from native
execution and must be addressed at the architectural level rather than
worked around at runtime.

### WASM Execution Environment Constraints

| Constraint | Detail | Impact on Quantum Simulation |
|------------|--------|------------------------------|
| 32-bit address space | ~4 GB theoretical max, ~2 GB practical | Hard ceiling on state vector size |
| Memory model | Linear memory, grows in 64 KB pages | Allocation must be page-aware |
| No native threads | Web Workers required for parallelism | Requires SharedArrayBuffer + COOP/COEP headers |
| No direct GPU | WebGPU is separate API, not WASM-native | GPU acceleration unavailable in WASM path |
| No OS syscalls | Sandboxed execution, no file/network | All I/O must go through host bindings |
| JIT compilation | V8/SpiderMonkey JIT, not AOT | ~1.5-3x slower than native, variable warmup |
| SIMD support | 128-bit SIMD proposal (widely supported since 2021) | 4 f32 or 2 f64 per vector lane |
| Stack size | Default ~1 MB, configurable | Deep recursion limited |

### Memory Budget Analysis for Quantum Simulation

The critical constraint is WASM's 32-bit address space. With a practical
usable limit of approximately 2 GB (due to browser memory allocation
behavior and address space fragmentation), the maximum feasible state vector
size is bounded:

```
Available WASM Memory Budget:

  Total addressable:     4,294,967,296 bytes  (4 GB theoretical)
  Practical usable:     ~2,147,483,648 bytes  (2 GB, browser-dependent)
  WASM overhead:          ~100,000,000 bytes  (module, stack, heap metadata)
  Application overhead:    ~50,000,000 bytes  (circuit data, scratch buffers)
  -------------------------------------------------
  Available for state:  ~2,000,000,000 bytes  (1.86 GB)

  State vector sizes:
    24 qubits:  268,435,456 bytes (256 MB)  -- comfortable
    25 qubits:  536,870,912 bytes (512 MB)  -- feasible
    25 + scratch: ~1,073,741,824 bytes       -- tight but within budget
    26 qubits: 1,073,741,824 bytes (1 GB)   -- state alone, no scratch room
    27 qubits: 2,147,483,648 bytes (2 GB)   -- exceeds practical limit
```

### Existing WASM Patterns in ruVector

The `ruvector-router-wasm` crate establishes conventions for WASM compilation:

- `wasm-pack build` as the compilation tool
- `wasm-bindgen` for JavaScript interop
- TypeScript definition generation
- Feature-flag controlled inclusion/exclusion of capabilities
- Dedicated test suites using `wasm-bindgen-test`

## Decision

### 1. Target and Toolchain

**Target triple**: `wasm32-unknown-unknown`

**Build toolchain**: `wasm-pack` with `wasm-bindgen`

```bash
# Development build
wasm-pack build crates/ruqu-wasm --target web --dev

# Release build with size optimization
wasm-pack build crates/ruqu-wasm --target web --release

# Node.js target (for server-side WASM)
wasm-pack build crates/ruqu-wasm --target nodejs --release
```

**Cargo profile for WASM release**:

```toml
[profile.wasm-release]
inherits = "release"
opt-level = "z"          # Optimize for binary size
lto = true               # Link-time optimization
codegen-units = 1        # Single codegen unit for maximum optimization
strip = true             # Strip debug symbols
panic = "abort"          # Smaller panic handling
```

### 2. Memory Limit Enforcement

`ruqu-wasm` enforces qubit limits before any allocation occurs. This is a hard
gate, not a soft warning.

**Enforcement strategy**:

```
User requests N qubits
        |
        v
  [N <= 25?] ---NO---> Return WasmLimitError {
        |                 requested: N,
       YES                maximum: 25,
        |                 estimated_memory: 16 * 2^N,
        v                 suggestion: "Use native build for >25 qubits"
  [Estimate total       }
   memory needed]
        |
        v
  [< 1.5 GB?] ---NO---> Return WasmLimitError::InsufficientMemory
        |
       YES
        |
        v
  Proceed with allocation
```

**Qubit limits by precision**:

| Precision | Max Qubits (WASM) | State Size | With Scratch |
|-----------|--------------------|------------|--------------|
| Complex f64 (default) | 25 | 512 MB | ~1.07 GB |
| Complex f32 (optional) | 26 | 512 MB | ~1.07 GB |

**Error reporting**:

```rust
#[wasm_bindgen]
#[derive(Debug)]
pub struct WasmLimitError {
    pub requested_qubits: usize,
    pub maximum_qubits: usize,
    pub estimated_bytes: usize,
    pub message: String,
}

impl WasmLimitError {
    pub fn qubit_overflow(requested: usize) -> Self {
        let max = if cfg!(feature = "f32") { 26 } else { 25 };
        let bytes_per_amplitude = if cfg!(feature = "f32") { 8 } else { 16 };
        Self {
            requested_qubits: requested,
            maximum_qubits: max,
            estimated_bytes: bytes_per_amplitude * (1usize << requested),
            message: format!(
                "Cannot simulate {} qubits in WASM: requires {} bytes, \
                 exceeds WASM address space. Maximum: {} qubits. \
                 Use native build for larger simulations.",
                requested,
                bytes_per_amplitude * (1usize << requested),
                max
            ),
        }
    }
}
```

### 3. Threading Strategy

WASM multi-threading requires SharedArrayBuffer, which in turn requires
specific HTTP security headers (Cross-Origin-Opener-Policy and
Cross-Origin-Embedder-Policy). Not all deployment environments support these.

**Strategy**: Optional multi-threading with graceful fallback.

```
                  ruqu-wasm execution
                        |
                        v
              [SharedArrayBuffer
               available?]
                /           \
              YES            NO
              /               \
    [wasm-bindgen-rayon]    [single-threaded
     parallel execution]     execution]
              |                    |
     Split state vector      Sequential gate
     across Web Workers      application
              |                    |
              v                    v
         Fast (N cores)     Slower (1 core)
```

**Compile-time configuration**:

```toml
# In ruqu-wasm/Cargo.toml
[features]
default = []
threads = ["wasm-bindgen-rayon", "ruqu-core/parallel"]
```

**Runtime detection**:

```rust
#[wasm_bindgen]
pub fn threading_available() -> bool {
    // Check if SharedArrayBuffer is available in this environment
    js_sys::eval("typeof SharedArrayBuffer !== 'undefined'")
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}
```

**Required HTTP headers for threading**:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

### 4. SIMD Utilization

The WASM SIMD proposal (128-bit vectors) is widely supported in modern browsers
and runtimes. The quantum engine uses SIMD for amplitude manipulation when
available.

**WASM SIMD capabilities**:

| Operation | WASM SIMD Instruction | Use in Quantum Sim |
|-----------|-----------------------|--------------------|
| f64x2 multiply | `f64x2.mul` | Complex multiplication (real part) |
| f64x2 add | `f64x2.add` | Amplitude accumulation |
| f64x2 sub | `f64x2.sub` | Complex multiplication (cross terms) |
| f64x2 shuffle | `i64x2.shuffle` | Swapping real/imaginary parts |
| f32x4 multiply | `f32x4.mul` | f32 mode complex multiply |
| f32x4 fma | emulated | Fused multiply-add for accuracy |

**Conditional compilation**:

```rust
// In ruqu-core, WASM SIMD path
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod wasm_simd {
    use core::arch::wasm32::*;

    /// Apply 2x2 unitary to a pair of amplitudes using WASM SIMD
    #[inline(always)]
    pub fn apply_gate_2x2_simd(
        a_re: f64, a_im: f64,
        b_re: f64, b_im: f64,
        u00_re: f64, u00_im: f64,
        u01_re: f64, u01_im: f64,
        u10_re: f64, u10_im: f64,
        u11_re: f64, u11_im: f64,
    ) -> (f64, f64, f64, f64) {
        // Pack amplitude pair into SIMD lanes
        let a = f64x2(a_re, a_im);
        let b = f64x2(b_re, b_im);

        // Complex multiply-accumulate for output amplitudes
        // c0 = u00*a + u01*b
        // c1 = u10*a + u11*b
        // (expanded for complex arithmetic)
        // ...
        todo!()
    }
}

// Fallback scalar path
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
mod scalar {
    // Pure scalar complex arithmetic
}
```

**Comparison of SIMD widths across targets**:

```
Native (AVX-512):  512-bit  =  8 f64  =  4 complex f64 per instruction
Native (AVX2):     256-bit  =  4 f64  =  2 complex f64 per instruction
Native (NEON):     128-bit  =  2 f64  =  1 complex f64 per instruction
WASM SIMD:         128-bit  =  2 f64  =  1 complex f64 per instruction
```

WASM SIMD matches ARM NEON width but is slower due to JIT overhead. The engine
uses the same algorithmic structure as the NEON path, adapted for WASM SIMD
intrinsics.

### 5. No GPU in WASM

GPU acceleration is exclusively available in native builds. The WASM path
uses CPU-only simulation.

**Rationale**:
- WebGPU is a separate browser API, not accessible from WASM linear memory
- Bridging WASM to WebGPU would require complex JavaScript glue code
- WebGPU compute shader support varies across browsers
- The performance benefit is uncertain for the 25-qubit WASM ceiling

**Future consideration**: If WebGPU stabilizes and WASM-WebGPU interop matures,
a `ruqu-webgpu` crate could provide browser-side GPU acceleration. This is out
of scope for the initial release.

### 6. API Parity

`ruqu-wasm` exposes an API that is functionally identical to `ruqu-core` native.
The same circuit description produces the same measurement results (within
floating-point tolerance). Only performance and capacity differ.

**Parity guarantee**:

```
                    Same Circuit
                        |
           +------------+------------+
           |                         |
     ruqu-core (native)       ruqu-wasm (browser)
           |                         |
    - 30+ qubits              - 25 qubits max
    - AVX2/AVX-512 SIMD       - WASM SIMD128
    - Rayon threading          - Optional Web Workers
    - Optional GPU             - CPU only
    - ~17.5M gates/sec         - ~5-12M gates/sec
           |                         |
           +------------+------------+
                        |
                  Same Results
              (within fp tolerance)
```

**Verified by**: Shared test suite that runs against both native and WASM targets,
comparing outputs bitwise (for deterministic operations) or statistically (for
measurement sampling).

### 7. Module Size Target

Target `.wasm` binary size: **< 2 MB** for the default feature set.

**Size budget**:

| Component | Estimated Size |
|-----------|---------------|
| Core simulation engine | ~800 KB |
| Gate implementations | ~200 KB |
| Measurement and sampling | ~100 KB |
| wasm-bindgen glue | ~50 KB |
| Circuit optimization | ~150 KB |
| Error handling and validation | ~50 KB |
| **Total (default features)** | **~1.35 MB** |
| + noise-model feature | +200 KB |
| + tensor-network feature | +400 KB |
| **Total (all features)** | **~1.95 MB** |

**Size reduction techniques**:
- `opt-level = "z"` for size-optimized compilation
- LTO (Link-Time Optimization) for dead code elimination
- `wasm-opt` post-processing pass (binaryen)
- Feature flags to exclude unused capabilities
- `panic = "abort"` to eliminate unwinding machinery
- Avoid `format!` and `std::fmt` where possible in hot paths

**Build pipeline**:

```bash
# Build with wasm-pack
wasm-pack build crates/ruqu-wasm --target web --release

# Post-process with wasm-opt for additional size reduction
wasm-opt -Oz --enable-simd \
    crates/ruqu-wasm/pkg/ruqu_wasm_bg.wasm \
    -o crates/ruqu-wasm/pkg/ruqu_wasm_bg.wasm

# Verify size
ls -lh crates/ruqu-wasm/pkg/ruqu_wasm_bg.wasm
# Expected: < 2 MB
```

### 8. Future: wasm64 (Memory64 Proposal)

The WebAssembly Memory64 proposal extends the address space to 64 bits,
removing the 4 GB limitation. When this proposal reaches broad runtime support:

- Recompile `ruqu-wasm` targeting `wasm64-unknown-unknown`
- Lift the 25-qubit ceiling to match native limits
- Maintain backward compatibility with wasm32 via conditional compilation

**Current status**: Memory64 is at Phase 4 (standardized) in the WASM
specification process. Browser support is emerging but not yet universal.

**Migration path**:

```toml
# Future Cargo.toml
[features]
wasm64 = []  # Enable when targeting wasm64

# In code
#[cfg(feature = "wasm64")]
const MAX_QUBITS_WASM: usize = 30;

#[cfg(not(feature = "wasm64"))]
const MAX_QUBITS_WASM: usize = 25;
```

## Trade-offs Accepted

| Trade-off | Accepted Limitation | Justification |
|-----------|---------------------|---------------|
| Performance | ~1.5-3x slower than native | Universal deployment outweighs raw speed |
| Qubit ceiling | 25 qubits in WASM vs 30+ native | Sufficient for most educational and research workloads |
| Threading | Requires specific browser headers | Graceful fallback ensures always-works baseline |
| No GPU | CPU-only in browser | GPU simulation at 25 qubits shows minimal benefit |
| Binary size | ~1.35 MB module | Acceptable for a quantum simulation library |

## Consequences

### Positive

- **Universal deployment**: Any modern browser or WASM runtime can execute
  quantum simulations without installation
- **Security sandboxing**: WASM's memory isolation prevents quantum simulation
  code from accessing host resources
- **Edge-aligned**: Matches ruVector's philosophy of computation at the edge
- **Testable**: WASM builds can be tested in CI via headless browsers and
  wasm-bindgen-test
- **Progressive enhancement**: Single-threaded baseline with optional threading
  ensures broad compatibility

### Negative

- **Performance ceiling**: JIT overhead and narrower SIMD limit throughput
- **Memory limits**: 25-qubit hard ceiling until wasm64 adoption
- **Threading complexity**: SharedArrayBuffer requirement adds deployment
  configuration burden
- **Debugging difficulty**: WASM debugging tools are less mature than native
  debuggers

### Mitigations

| Issue | Mitigation |
|-------|------------|
| Performance gap | Document native vs WASM trade-offs; recommend native for >20 qubits |
| Memory exhaustion | Hard limit enforcement with informative error messages |
| Threading failures | Automatic fallback to single-threaded; no silent degradation |
| Debug difficulty | Source maps via wasm-pack; comprehensive logging to console |
| Binary size creep | CI size gate: fail build if .wasm exceeds 2 MB |

## References

- [ADR-QE-001: Quantum Engine Core Architecture](./ADR-QE-001-quantum-engine-core-architecture.md)
- [ADR-QE-002: Crate Structure & Integration](./ADR-QE-002-crate-structure-integration.md)
- [ADR-QE-004: Performance Optimization & Benchmarks](./ADR-QE-004-performance-optimization-benchmarks.md)
- [ADR-005: WASM Runtime Integration](/docs/adr/ADR-005-wasm-runtime-integration.md)
- [ruvector-router-wasm crate](/crates/ruvector-router-wasm/)
- [WebAssembly SIMD Proposal](https://github.com/WebAssembly/simd)
- [WebAssembly Memory64 Proposal](https://github.com/WebAssembly/memory64)
- [wasm-bindgen-rayon](https://github.com/RReverser/wasm-bindgen-rayon)
- [Cross-Origin Isolation Guide (MDN)](https://developer.mozilla.org/en-US/docs/Web/API/crossOriginIsolated)
