# ADR-005: WASM Runtime Integration

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-01-18 |
| **Authors** | RuvLLM Architecture Team |
| **Reviewers** | - |
| **Supersedes** | - |
| **Superseded by** | - |

**Note**: The WASM runtime approach described here is complemented by ADR-029. The RVF WASM microkernel (rvf-wasm) provides a <8 KB Cognitum tile target that replaces ad-hoc WASM builds for vector operations.

## 1. Context

### 1.1 Problem Statement

RuvLLM requires a mechanism for executing user-provided and community-contributed compute kernels in a secure, sandboxed environment. These kernels implement performance-critical operations such as:

- Rotary Position Embeddings (RoPE)
- RMS Normalization (RMSNorm)
- SwiGLU activation functions
- KV cache quantization/dequantization
- LoRA delta application

Without proper isolation, malicious or buggy kernels could:
- Access unauthorized memory regions
- Consume unbounded compute resources
- Compromise the host system
- Corrupt model state

### 1.2 Requirements

| Requirement | Priority | Rationale |
|-------------|----------|-----------|
| Sandboxed execution | Critical | Prevent kernel code from accessing host resources |
| Execution budgets | Critical | Prevent runaway code and DoS conditions |
| Low overhead | High | Kernels are in the inference hot path |
| Cross-platform | High | Support x86, ARM, embedded devices |
| Framework agnostic | Medium | Enable ML inference without vendor lock-in |
| Hot-swappable kernels | Medium | Update kernels without service restart |

### 1.3 Constraints

- **Memory**: Embedded targets have as little as 256KB RAM
- **Latency**: Kernel invocation overhead must be <10us for small tensors
- **Compatibility**: Must support existing Rust/C kernel implementations
- **Security**: Kernel supply chain must be verifiable

## 2. Decision

We will adopt **WebAssembly (WASM)** as the sandboxed execution environment for compute kernels, with the following architecture:

### 2.1 Runtime Selection

| Device Class | Runtime | Rationale |
|--------------|---------|-----------|
| Edge servers (x86/ARM64) | **Wasmtime** | Mature, well-optimized, excellent tooling |
| Embedded/MCU (<1MB RAM) | **WAMR** | <85KB footprint, AOT compilation support |
| Browser/WASI Preview 2 | **wasmtime/browser** | Future consideration |

### 2.2 Interruption Strategy: Epoch-Based (Not Fuel)

We choose **epoch-based interruption** over fuel-based metering:

| Aspect | Epoch | Fuel |
|--------|-------|------|
| Overhead | ~2-5% | ~15-30% |
| Granularity | Coarse (polling points) | Fine (per instruction) |
| Determinism | Non-deterministic | Deterministic |
| Implementation | Store-level epoch counter | Instruction instrumentation |

**Rationale**: For inference workloads, coarse-grained interruption is acceptable. The 10-25% overhead reduction from avoiding fuel metering is significant for latency-sensitive operations.

```rust
// Epoch configuration example
let mut config = Config::new();
config.epoch_interruption(true);

let engine = Engine::new(&config)?;
let mut store = Store::new(&engine, ());

// Set epoch deadline (e.g., 100ms budget)
store.set_epoch_deadline(100);

// Increment epoch from async timer
engine.increment_epoch();
```

### 2.3 WASI-NN Integration

WASI-NN provides framework-agnostic ML inference capabilities:

```
+-------------------+
|   RuvLLM Host     |
+-------------------+
         |
         v
+-------------------+
|   WASI-NN API     |
+-------------------+
         |
    +----+----+
    |         |
    v         v
+-------+ +--------+
| ONNX  | | Custom |
| RT    | | Kernel |
+-------+ +--------+
```

**WASI-NN Backends**:
- ONNX Runtime (portable)
- Native kernels (performance-critical paths)
- Custom quantized formats (memory efficiency)

## 3. WASM Boundary Design

### 3.1 ABI Strategy: Raw ABI (Not Component Model)

We use **raw WASM ABI** rather than the Component Model:

| Aspect | Raw ABI | Component Model |
|--------|---------|-----------------|
| Maturity | Stable | Evolving (Preview 2) |
| Overhead | Minimal | Higher (canonical ABI) |
| Tooling | Excellent | Improving |
| Adoption | Universal | Growing |

**Migration Path**: Design interfaces to be Component Model-compatible for future migration.

### 3.2 Memory Layout

```
Host Linear Memory
+--------------------------------------------------+
| Tensor A    | Tensor B    | Output    | Scratch  |
| (read-only) | (read-only) | (write)   | (r/w)    |
+--------------------------------------------------+
     ^              ^            ^           ^
     |              |            |           |
   offset_a     offset_b    offset_out   offset_scratch
```

**Shared Memory Protocol**:

```rust
/// Kernel invocation descriptor passed to WASM
#[repr(C)]
pub struct KernelDescriptor {
    /// Input tensor A offset in linear memory
    pub input_a_offset: u32,
    /// Input tensor A size in bytes
    pub input_a_size: u32,
    /// Input tensor B offset (0 if unused)
    pub input_b_offset: u32,
    /// Input tensor B size in bytes
    pub input_b_size: u32,
    /// Output tensor offset
    pub output_offset: u32,
    /// Output tensor size in bytes
    pub output_size: u32,
    /// Scratch space offset
    pub scratch_offset: u32,
    /// Scratch space size in bytes
    pub scratch_size: u32,
    /// Kernel-specific parameters offset
    pub params_offset: u32,
    /// Kernel-specific parameters size
    pub params_size: u32,
}
```

### 3.3 Trap Handling

WASM traps are handled as **non-fatal errors**:

```rust
pub enum KernelError {
    /// Execution budget exceeded
    EpochDeadline,
    /// Out of bounds memory access
    MemoryAccessViolation {
        offset: u32,
        size: u32,
    },
    /// Integer overflow/underflow
    IntegerOverflow,
    /// Unreachable code executed
    Unreachable,
    /// Stack overflow
    StackOverflow,
    /// Invalid function call
    IndirectCallTypeMismatch,
    /// Custom trap from kernel
    KernelTrap {
        code: u32,
        message: Option<String>,
    },
}

impl From<wasmtime::Trap> for KernelError {
    fn from(trap: wasmtime::Trap) -> Self {
        match trap.trap_code() {
            Some(TrapCode::Interrupt) => KernelError::EpochDeadline,
            Some(TrapCode::MemoryOutOfBounds) => KernelError::MemoryAccessViolation {
                offset: 0, // Extract from trap info
                size: 0,
            },
            // ... other mappings
        }
    }
}
```

**Recovery Strategy**:

1. Log trap with full context
2. Release kernel resources
3. Fall back to reference implementation (if available)
4. Report degraded performance to metrics

## 4. Kernel Pack System

### 4.1 Kernel Pack Structure

```
kernel-pack-v1.0.0/
├── kernels.json          # Manifest
├── kernels.json.sig      # Ed25519 signature
├── rope/
│   ├── rope_f32.wasm
│   ├── rope_f16.wasm
│   └── rope_q8.wasm
├── rmsnorm/
│   ├── rmsnorm_f32.wasm
│   └── rmsnorm_f16.wasm
├── swiglu/
│   ├── swiglu_f32.wasm
│   └── swiglu_f16.wasm
├── kv/
│   ├── kv_pack_q4.wasm
│   ├── kv_pack_q8.wasm
│   ├── kv_unpack_q4.wasm
│   └── kv_unpack_q8.wasm
└── lora/
    ├── lora_apply_f32.wasm
    └── lora_apply_f16.wasm
```

### 4.2 Manifest Schema (kernels.json)

```json
{
  "$schema": "https://ruvllm.dev/schemas/kernel-pack-v1.json",
  "version": "1.0.0",
  "name": "ruvllm-core-kernels",
  "description": "Core compute kernels for RuvLLM inference",
  "min_runtime_version": "0.5.0",
  "max_runtime_version": "1.0.0",
  "created_at": "2026-01-18T00:00:00Z",
  "author": {
    "name": "RuvLLM Team",
    "email": "kernels@ruvllm.dev",
    "signing_key": "ed25519:AAAA..."
  },
  "kernels": [
    {
      "id": "rope_f32",
      "name": "Rotary Position Embedding (FP32)",
      "category": "positional_encoding",
      "path": "rope/rope_f32.wasm",
      "hash": "sha256:abc123...",
      "entry_point": "rope_forward",
      "inputs": [
        {
          "name": "x",
          "dtype": "f32",
          "shape": ["batch", "seq", "heads", "dim"]
        },
        {
          "name": "freqs",
          "dtype": "f32",
          "shape": ["seq", "dim_half"]
        }
      ],
      "outputs": [
        {
          "name": "y",
          "dtype": "f32",
          "shape": ["batch", "seq", "heads", "dim"]
        }
      ],
      "params": {
        "theta": {
          "type": "f32",
          "default": 10000.0
        }
      },
      "resource_limits": {
        "max_memory_pages": 256,
        "max_epoch_ticks": 1000,
        "max_table_elements": 1024
      },
      "platforms": {
        "wasmtime": {
          "min_version": "15.0.0",
          "features": ["simd", "bulk-memory"]
        },
        "wamr": {
          "min_version": "1.3.0",
          "aot_available": true
        }
      },
      "benchmarks": {
        "seq_512_dim_128": {
          "latency_us": 45,
          "throughput_gflops": 2.1
        }
      }
    }
  ],
  "fallbacks": {
    "rope_f32": "rope_reference",
    "rmsnorm_f32": "rmsnorm_reference"
  }
}
```

### 4.3 Included Kernel Packs

| Category | Kernels | Notes |
|----------|---------|-------|
| **Positional** | RoPE (f32, f16, q8) | Rotary embeddings |
| **Normalization** | RMSNorm (f32, f16) | Pre-attention normalization |
| **Activation** | SwiGLU (f32, f16) | Gated activation |
| **KV Cache** | pack_q4, pack_q8, unpack_q4, unpack_q8 | Quantize/dequantize |
| **Adapter** | LoRA apply (f32, f16) | Delta weight application |

**Attention Note**: Attention kernels remain **native** initially due to:
- Complex memory access patterns
- Heavy reliance on hardware-specific optimizations (Flash Attention, xformers)
- Significant overhead from WASM boundary crossing for large tensors

## 5. Supply Chain Security

### 5.1 Signature Verification

```rust
use ed25519_dalek::{Signature, VerifyingKey, Verifier};

pub struct KernelPackVerifier {
    trusted_keys: Vec<VerifyingKey>,
}

impl KernelPackVerifier {
    /// Verify kernel pack signature
    pub fn verify(&self, manifest: &[u8], signature: &[u8]) -> Result<(), VerifyError> {
        let sig = Signature::try_from(signature)?;

        for key in &self.trusted_keys {
            if key.verify(manifest, &sig).is_ok() {
                return Ok(());
            }
        }

        Err(VerifyError::NoTrustedKey)
    }

    /// Verify individual kernel hash
    pub fn verify_kernel(&self, kernel_bytes: &[u8], expected_hash: &str) -> Result<(), VerifyError> {
        use sha2::{Sha256, Digest};

        let mut hasher = Sha256::new();
        hasher.update(kernel_bytes);
        let hash = format!("sha256:{:x}", hasher.finalize());

        if hash == expected_hash {
            Ok(())
        } else {
            Err(VerifyError::HashMismatch {
                expected: expected_hash.to_string(),
                actual: hash,
            })
        }
    }
}
```

### 5.2 Version Compatibility Gates

```rust
pub struct CompatibilityChecker {
    runtime_version: Version,
}

impl CompatibilityChecker {
    pub fn check(&self, manifest: &KernelManifest) -> CompatibilityResult {
        // Check runtime version bounds
        if self.runtime_version < manifest.min_runtime_version {
            return CompatibilityResult::RuntimeTooOld {
                required: manifest.min_runtime_version.clone(),
                actual: self.runtime_version.clone(),
            };
        }

        if self.runtime_version > manifest.max_runtime_version {
            return CompatibilityResult::RuntimeTooNew {
                max_supported: manifest.max_runtime_version.clone(),
                actual: self.runtime_version.clone(),
            };
        }

        // Check WASM feature requirements
        for kernel in &manifest.kernels {
            if let Some(platform) = kernel.platforms.get("wasmtime") {
                for feature in &platform.features {
                    if !self.has_feature(feature) {
                        return CompatibilityResult::MissingFeature {
                            kernel: kernel.id.clone(),
                            feature: feature.clone(),
                        };
                    }
                }
            }
        }

        CompatibilityResult::Compatible
    }
}
```

### 5.3 Safe Rollback Protocol

```rust
pub struct KernelManager {
    active_pack: Arc<RwLock<KernelPack>>,
    previous_pack: Arc<RwLock<Option<KernelPack>>>,
    metrics: KernelMetrics,
}

impl KernelManager {
    /// Upgrade to new kernel pack with automatic rollback on failure
    pub async fn upgrade(&self, new_pack: KernelPack) -> Result<(), UpgradeError> {
        // Step 1: Verify new pack
        self.verifier.verify(&new_pack)?;
        self.compatibility.check(&new_pack.manifest)?;

        // Step 2: Compile kernels (AOT if supported)
        let compiled = self.compile_pack(&new_pack).await?;

        // Step 3: Atomic swap with rollback capability
        {
            let mut active = self.active_pack.write().await;
            let mut previous = self.previous_pack.write().await;

            // Store current as rollback target
            *previous = Some(std::mem::replace(&mut *active, compiled));
        }

        // Step 4: Health check with new kernels
        if let Err(e) = self.health_check().await {
            tracing::error!("Kernel health check failed: {}", e);
            self.rollback().await?;
            return Err(UpgradeError::HealthCheckFailed(e));
        }

        // Step 5: Clear rollback after grace period
        tokio::spawn({
            let previous = self.previous_pack.clone();
            async move {
                tokio::time::sleep(Duration::from_secs(300)).await;
                *previous.write().await = None;
            }
        });

        Ok(())
    }

    /// Rollback to previous kernel pack
    pub async fn rollback(&self) -> Result<(), RollbackError> {
        let mut active = self.active_pack.write().await;
        let mut previous = self.previous_pack.write().await;

        if let Some(prev) = previous.take() {
            *active = prev;
            tracing::info!("Rolled back to previous kernel pack");
            Ok(())
        } else {
            Err(RollbackError::NoPreviousPack)
        }
    }
}
```

## 6. Device Class Configurations

### 6.1 Edge Server Configuration (Wasmtime + Epoch)

```rust
pub fn create_server_runtime() -> Result<WasmRuntime, RuntimeError> {
    let mut config = Config::new();

    // Performance optimizations
    config.cranelift_opt_level(OptLevel::Speed);
    config.cranelift_nan_canonicalization(false);
    config.parallel_compilation(true);

    // SIMD support for vectorized operations
    config.wasm_simd(true);
    config.wasm_bulk_memory(true);
    config.wasm_multi_value(true);

    // Memory configuration
    config.static_memory_maximum_size(1 << 32); // 4GB max
    config.dynamic_memory_guard_size(1 << 16);  // 64KB guard

    // Epoch-based interruption
    config.epoch_interruption(true);

    let engine = Engine::new(&config)?;

    Ok(WasmRuntime {
        engine,
        epoch_tick_interval: Duration::from_millis(10),
        default_epoch_budget: 1000, // 10 seconds max
    })
}
```

### 6.2 Embedded Configuration (WAMR AOT)

```rust
pub fn create_embedded_runtime() -> Result<WamrRuntime, RuntimeError> {
    let mut config = WamrConfig::new();

    // Minimal footprint configuration
    config.set_stack_size(32 * 1024);        // 32KB stack
    config.set_heap_size(128 * 1024);        // 128KB heap
    config.enable_aot(true);                  // Pre-compiled modules
    config.enable_simd(false);                // Often unavailable on MCU
    config.enable_bulk_memory(true);

    // Interpreter fallback for debugging
    config.enable_interp(cfg!(debug_assertions));

    // Execution limits
    config.set_exec_timeout_ms(100);          // 100ms max per invocation

    Ok(WamrRuntime::new(config)?)
}
```

### 6.3 WASI Threads (Optional)

For platforms supporting WASI threads:

```rust
pub fn create_threaded_runtime() -> Result<WasmRuntime, RuntimeError> {
    let mut config = Config::new();

    // Enable threading support
    config.wasm_threads(true);
    config.wasm_shared_memory(true);

    // Thread pool configuration
    config.async_support(true);
    config.max_wasm_threads(4);

    let engine = Engine::new(&config)?;

    Ok(WasmRuntime {
        engine,
        thread_pool_size: 4,
    })
}
```

**Platform Support Matrix**:

| Platform | WASI Threads | Notes |
|----------|--------------|-------|
| Linux x86_64 | Yes | Full support |
| Linux ARM64 | Yes | Full support |
| macOS | Yes | Full support |
| Windows | Yes | Full support |
| WAMR | No | Single-threaded only |
| Browser | Yes | Via SharedArrayBuffer |

## 7. Performance Considerations

### 7.1 Invocation Overhead

| Operation | Latency | Notes |
|-----------|---------|-------|
| Kernel lookup | ~100ns | Hash table lookup |
| Instance creation | ~1us | Pre-compiled module |
| Memory setup | ~500ns | Shared memory mapping |
| Epoch check | ~2ns | Single atomic read |
| Return value | ~100ns | Register transfer |
| **Total** | **~2us** | Per invocation |

### 7.2 Optimization Strategies

1. **Module Caching**: Pre-compile and cache WASM modules
2. **Instance Pooling**: Reuse instances across invocations
3. **Memory Sharing**: Map host tensors directly into WASM linear memory
4. **Batch Invocations**: Process multiple requests per kernel call

### 7.3 When to Bypass WASM

WASM sandboxing should be bypassed (with explicit opt-in) for:

- Attention kernels (complex memory patterns)
- Large matrix multiplications (>1000x1000)
- Operations with <1ms latency requirements
- Trusted, verified native kernels

## 8. Alternatives Considered

### 8.1 eBPF

| Aspect | eBPF | WASM |
|--------|------|------|
| Platform | Linux only | Cross-platform |
| Verification | Static, strict | Dynamic, flexible |
| Memory model | Constrained | Linear memory |
| Tooling | Improving | Mature |

**Decision**: WASM chosen for cross-platform support.

### 8.2 Lua/LuaJIT

| Aspect | Lua | WASM |
|--------|-----|------|
| Performance | Good (JIT) | Excellent (AOT) |
| Sandboxing | Manual effort | Built-in |
| Type safety | Dynamic | Static |
| Ecosystem | Large | Growing |

**Decision**: WASM chosen for type safety and native compilation.

### 8.3 Native Plugins with seccomp

| Aspect | seccomp | WASM |
|--------|---------|------|
| Isolation | Process-level | In-process |
| Overhead | IPC cost | Minimal |
| Portability | Linux only | Cross-platform |
| Complexity | High | Moderate |

**Decision**: WASM chosen for in-process efficiency and portability.

## 9. Consequences

### 9.1 Positive

- **Security**: Strong isolation prevents kernel code from compromising host
- **Portability**: Same kernels run on servers and embedded devices
- **Hot Updates**: Kernels can be updated without service restart
- **Ecosystem**: Large WASM toolchain and community support
- **Auditability**: WASM modules can be inspected and verified

### 9.2 Negative

- **Overhead**: ~2us per invocation vs. native direct call
- **Complexity**: Additional abstraction layer to maintain
- **Tooling**: WASM debugging tools less mature than native
- **Learning Curve**: Team needs WASM expertise

### 9.3 Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Performance regression | Medium | High | Benchmark suite, native fallbacks |
| WASI-NN instability | Low | Medium | Abstract behind internal API |
| Supply chain attack | Low | Critical | Signature verification, trusted keys |
| Epoch timing variability | Medium | Low | Generous budgets, monitoring |

## 10. Implementation Plan

### Phase 1: Foundation (Weeks 1-2)
- [ ] Set up Wasmtime integration
- [ ] Implement kernel descriptor ABI
- [ ] Create basic kernel loader

### Phase 2: Core Kernels (Weeks 3-4)
- [ ] Implement RoPE kernel
- [ ] Implement RMSNorm kernel
- [ ] Implement SwiGLU kernel

### Phase 3: KV Cache (Weeks 5-6)
- [ ] Implement quantization kernels
- [ ] Implement dequantization kernels
- [ ] Integration with cache manager

### Phase 4: Security (Weeks 7-8)
- [ ] Implement signature verification
- [ ] Create version compatibility checker
- [ ] Build rollback system

### Phase 5: Embedded (Weeks 9-10)
- [ ] WAMR integration
- [ ] AOT compilation pipeline
- [ ] Resource-constrained testing

## 11. References

- [Wasmtime Documentation](https://docs.wasmtime.dev/)
- [WAMR Documentation](https://github.com/bytecodealliance/wasm-micro-runtime)
- [WASI-NN Specification](https://github.com/WebAssembly/wasi-nn)
- [WebAssembly Security Model](https://webassembly.org/docs/security/)
- [Component Model Proposal](https://github.com/WebAssembly/component-model)

## 12. Appendix

### A. Kernel Interface Definition

```rust
/// Standard kernel interface (exported by WASM modules)
#[link(wasm_import_module = "ruvllm")]
extern "C" {
    /// Initialize kernel with parameters
    fn kernel_init(params_ptr: *const u8, params_len: u32) -> i32;

    /// Execute kernel forward pass
    fn kernel_forward(desc_ptr: *const KernelDescriptor) -> i32;

    /// Execute kernel backward pass (optional)
    fn kernel_backward(desc_ptr: *const KernelDescriptor) -> i32;

    /// Get kernel metadata
    fn kernel_info(info_ptr: *mut KernelInfo) -> i32;

    /// Cleanup kernel resources
    fn kernel_cleanup() -> i32;
}
```

### B. Error Codes

| Code | Name | Description |
|------|------|-------------|
| 0 | OK | Success |
| 1 | INVALID_INPUT | Invalid input tensor |
| 2 | INVALID_OUTPUT | Invalid output tensor |
| 3 | INVALID_PARAMS | Invalid kernel parameters |
| 4 | OUT_OF_MEMORY | Insufficient memory |
| 5 | NOT_IMPLEMENTED | Operation not supported |
| 6 | INTERNAL_ERROR | Internal kernel error |

### C. Benchmark Template

```rust
#[cfg(test)]
mod benchmarks {
    use criterion::{criterion_group, criterion_main, Criterion};

    fn bench_rope_f32(c: &mut Criterion) {
        let runtime = create_server_runtime().unwrap();
        let kernel = runtime.load_kernel("rope_f32").unwrap();

        let input = Tensor::random([1, 512, 32, 128], DType::F32);
        let freqs = Tensor::random([512, 64], DType::F32);

        c.bench_function("rope_f32_seq512", |b| {
            b.iter(|| {
                kernel.forward(&input, &freqs).unwrap()
            })
        });
    }

    criterion_group!(benches, bench_rope_f32);
    criterion_main!(benches);
}
```

---

## Related Decisions

- **ADR-001**: Ruvector Core Architecture
- **ADR-002**: RuvLLM Integration
- **ADR-003**: SIMD Optimization Strategy
- **ADR-007**: Security Review & Technical Debt

---

## Security Status (v2.1)

| Component | Status | Notes |
|-----------|--------|-------|
| SharedArrayBuffer | ✅ Secure | Safety documentation for race conditions |
| WASM Memory | ✅ Secure | Bounds checking via WASM sandbox |
| Kernel Loading | ⚠️ Planned | Signature verification pending |

**Fixes Applied:**
- Added comprehensive safety comments documenting race condition prevention in `shared.rs`
- JavaScript/WASM coordination patterns documented

**Outstanding Items:**
- TD-007 (P2): Embedded JavaScript should be extracted to separate files

See ADR-007 for full security audit trail.

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-18 | RuVector Architecture Team | Initial version |
| 1.1 | 2026-01-19 | Security Review Agent | Added security status, related decisions |
