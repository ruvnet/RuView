# ADR-050: Graph Transformer WASM and Node.js Bindings

## Status

Accepted

## Date

2026-02-25

## Context

RuVector's existing crates ship WASM and Node.js bindings following a consistent pattern: a `-wasm` crate using `wasm-bindgen` and a `-node` crate using `napi-rs`. Examples include `ruvector-gnn-wasm` / `ruvector-gnn-node`, `ruvector-graph-wasm` / `ruvector-graph-node`, `ruvector-verified-wasm`, and `ruvector-mincut-wasm` / `ruvector-mincut-node`.

The new `ruvector-graph-transformer` crate (ADR-046) needs equivalent bindings so that TypeScript/JavaScript applications can use proof-gated graph transformers in the browser (WASM) and on the server (Node.js via NAPI-RS). The challenge is deciding which subset of the Rust API to expose, managing the WASM binary size (target < 300 KB), and ensuring feature parity where feasible.

### Existing Binding Patterns

From `crates/ruvector-gnn-wasm/Cargo.toml`:
- `crate-type = ["cdylib", "rlib"]`
- Dependencies: `ruvector-gnn` with `default-features = false, features = ["wasm"]`
- Uses `serde-wasm-bindgen = "0.6"` for struct serialization
- Release profile: `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`

From `crates/ruvector-gnn-node/Cargo.toml`:
- `crate-type = ["cdylib"]`
- Dependencies: `napi = { workspace = true }`, `napi-derive = { workspace = true }`
- Build dependency: `napi-build = "2"`
- Release profile: `lto = true`, `strip = true`

From `crates/ruvector-verified-wasm/Cargo.toml`:
- Dependencies: `ruvector-verified` with `features = ["ultra"]`
- Uses `wasm-bindgen`, `serde-wasm-bindgen`, `js-sys`, `web-sys`
- Release profile: `opt-level = "s"`, `lto = true`

## Decision

We will create two binding crates following the established workspace patterns:

- `crates/ruvector-graph-transformer-wasm/` -- WASM bindings via `wasm-bindgen`
- `crates/ruvector-graph-transformer-node/` -- Node.js bindings via `napi-rs` (v2.16)

### API Surface: What to Expose

Not all Rust functionality translates efficiently to WASM/JS. The binding surface is scoped to three tiers:

**Tier 1 -- Core (both WASM and Node.js)**:
| API | Rust Source | Binding |
|-----|------------|---------|
| `GraphTransformer::new(config)` | `lib.rs` | Constructor, takes JSON config |
| `GraphTransformer::forward(batch)` | `lib.rs` | Returns `ProofGatedOutput` as JSON |
| `GraphTransformer::mutate(op)` | `lib.rs` | Returns mutation result + attestation |
| `ProofGate::unlock()` | `proof_gated/gate.rs` | Unlocks and returns inner value |
| `ProofGate::is_satisfied()` | `proof_gated/gate.rs` | Boolean check |
| `proof_chain()` | `proof_gated/mod.rs` | Returns attestation array as `Uint8Array[]` |
| `coherence()` | via `ruvector-coherence` | Returns coherence snapshot as JSON |

**Tier 2 -- Attention (both WASM and Node.js)**:
| API | Rust Source | Binding |
|-----|------------|---------|
| `PprSampledAttention::new()` | `sublinear_attention/ppr.rs` | Constructor |
| `LshSpectralAttention::new()` | `sublinear_attention/lsh.rs` | Constructor |
| `certify_complexity()` | `sublinear_attention/mod.rs` | Returns complexity bound as JSON |
| `SpectralSparsifier::sparsify()` | `sublinear_attention/spectral_sparsify.rs` | Returns sparsified edge list |

**Tier 3 -- Training (Node.js only, not WASM)**:
| API | Rust Source | Binding |
|-----|------------|---------|
| `VerifiedTrainer::new(config)` | `verified_training/pipeline.rs` | Constructor |
| `VerifiedTrainer::step()` | `verified_training/pipeline.rs` | Single training step |
| `VerifiedTrainer::seal()` | `verified_training/pipeline.rs` | Returns `TrainingCertificate` as JSON |
| `RobustnessCertifier::certify()` | `verified_training/mod.rs` | Returns certificate as JSON |

Training is excluded from WASM because:
1. Training requires `rayon` for parallelism (not available in WASM)
2. `ElasticWeightConsolidation` uses `ndarray` with BLAS, which adds ~500 KB to WASM size
3. Training workloads are server-side; inference is the browser use case

### WASM Crate Structure

```
crates/ruvector-graph-transformer-wasm/
  Cargo.toml
  src/
    lib.rs              # wasm_bindgen entry points
    types.rs            # TS-friendly wrapper types (JsValue serialization)
    proof_gate.rs       # ProofGate WASM bindings
    attention.rs        # Sublinear attention WASM bindings
    error.rs            # Error conversion to JsValue
  tests/
    web.rs              # wasm-bindgen-test integration tests
  package.json          # npm package metadata
  tsconfig.json         # TypeScript configuration for generated types
```

```toml
# Cargo.toml
[package]
name = "ruvector-graph-transformer-wasm"
version = "2.0.4"
edition = "2021"
rust-version = "1.77"
license = "MIT"
description = "WASM bindings for ruvector-graph-transformer: proof-gated graph transformers in the browser"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
ruvector-graph-transformer = { version = "2.0.4", path = "../ruvector-graph-transformer",
    default-features = false,
    features = ["proof-gated", "sublinear-attention"] }
wasm-bindgen = { workspace = true }
serde-wasm-bindgen = "0.6"
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
js-sys = { workspace = true }
web-sys = { workspace = true, features = ["console"] }
getrandom = { workspace = true, features = ["wasm_js"] }

[dev-dependencies]
wasm-bindgen-test = "0.3"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"

[profile.release.package."*"]
opt-level = "z"
```

### WASM Binding Implementation

```rust
// src/lib.rs
use wasm_bindgen::prelude::*;
use ruvector_graph_transformer::{GraphTransformer, GraphTransformerConfig};

#[wasm_bindgen]
pub struct WasmGraphTransformer {
    inner: GraphTransformer,
}

#[wasm_bindgen]
impl WasmGraphTransformer {
    /// Create a new graph transformer from JSON config.
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: &str) -> Result<WasmGraphTransformer, JsValue> {
        let config: GraphTransformerConfig = serde_json::from_str(config_json)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let inner = GraphTransformer::new(config, DefaultPropertyGraph::new())
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self { inner })
    }

    /// Run forward pass. Input and output are JSON-serialized.
    pub fn forward(&mut self, batch_json: &str) -> Result<JsValue, JsValue> {
        // Deserialize, run forward, serialize result
        // ...
    }

    /// Get the proof attestation chain as an array of Uint8Arrays.
    pub fn proof_chain(&self) -> Result<JsValue, JsValue> {
        let chain = self.inner.proof_chain();
        let array = js_sys::Array::new();
        for att in chain {
            let bytes = att.to_bytes();
            let uint8 = js_sys::Uint8Array::from(&bytes[..]);
            array.push(&uint8);
        }
        Ok(array.into())
    }

    /// Get coherence snapshot as JSON.
    pub fn coherence(&self) -> Result<String, JsValue> {
        let snapshot = self.inner.coherence();
        serde_json::to_string(&snapshot)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
```

### Node.js Crate Structure

```
crates/ruvector-graph-transformer-node/
  Cargo.toml
  src/
    lib.rs              # napi-rs entry points
    types.rs            # NAPI-RS type wrappers
    proof_gate.rs       # ProofGate Node bindings
    attention.rs        # Sublinear attention Node bindings
    training.rs         # VerifiedTrainer Node bindings (Tier 3)
  build.rs              # napi-build
  index.d.ts            # TypeScript type declarations
  package.json          # npm package metadata
  __test__/
    index.spec.mjs      # Node.js integration tests
```

```toml
# Cargo.toml
[package]
name = "ruvector-graph-transformer-node"
version = "2.0.4"
edition = "2021"
rust-version = "1.77"
license = "MIT"
description = "Node.js bindings for ruvector-graph-transformer via NAPI-RS"

[lib]
crate-type = ["cdylib"]

[dependencies]
ruvector-graph-transformer = { version = "2.0.4", path = "../ruvector-graph-transformer",
    features = ["full"] }
napi = { workspace = true }
napi-derive = { workspace = true }
serde_json = { workspace = true }

[build-dependencies]
napi-build = "2"

[profile.release]
lto = true
strip = true
```

### Node.js Binding Implementation (Training Example)

```rust
// src/training.rs
use napi::bindgen_prelude::*;
use napi_derive::napi;
use ruvector_graph_transformer::verified_training::{
    VerifiedTrainer, VerifiedTrainerConfig, TrainingCertificate,
};

#[napi(object)]
pub struct JsTrainingCertificate {
    pub total_steps: u32,
    pub violations: u32,
    pub final_loss: f64,
    pub final_coherence: Option<f64>,
    pub attestation_hex: String,
}

#[napi]
pub struct NodeVerifiedTrainer {
    inner: VerifiedTrainer,
}

#[napi]
impl NodeVerifiedTrainer {
    #[napi(constructor)]
    pub fn new(config_json: String) -> Result<Self> {
        let config: VerifiedTrainerConfig = serde_json::from_str(&config_json)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        let inner = VerifiedTrainer::new(config)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(Self { inner })
    }

    #[napi]
    pub fn step(&mut self, loss: f64, gradients_json: String) -> Result<String> {
        // Deserialize gradients, run step, serialize attestation
        // ...
    }

    #[napi]
    pub fn seal(&mut self) -> Result<JsTrainingCertificate> {
        // Seal training run and return certificate
        // ...
    }
}
```

### WASM Size Budget

Target: < 300 KB for the release `.wasm` binary (gzipped).

Size breakdown estimate:
| Component | Estimated Size |
|-----------|---------------|
| `ruvector-verified` (proof gates, arena, attestations) | ~40 KB |
| `ruvector-solver` (forward-push, random-walk, neumann) | ~60 KB |
| `ruvector-attention` (core attention only, no training) | ~80 KB |
| `ruvector-coherence` (metrics, no spectral) | ~15 KB |
| `wasm-bindgen` glue | ~20 KB |
| Serde JSON | ~50 KB |
| **Total (estimated)** | ~265 KB |

Size is controlled by:
1. `opt-level = "z"` (optimize for size)
2. `lto = true` (dead code elimination across crates)
3. `panic = "abort"` (no unwinding machinery)
4. `default-features = false` on `ruvector-graph-transformer` (only `proof-gated` and `sublinear-attention`)
5. Excluding training and the `spectral` feature from `ruvector-coherence`

If the target is exceeded, further reductions:
- Replace `serde_json` with `miniserde` (-30 KB)
- Strip `tracing` instrumentation via feature flag (-10 KB)
- Use `wasm-opt -Oz` post-processing (-10-20%)

### TypeScript Types

Both packages ship TypeScript type declarations. The WASM package generates types via `wasm-bindgen`'s `--typescript` flag. The Node.js package uses `napi-rs`'s automatic `.d.ts` generation from `#[napi]` attributes.

Key TypeScript interfaces:

```typescript
// Generated by wasm-bindgen / napi-rs

export interface GraphTransformerConfig {
  proofGated: boolean;
  attentionMechanism: 'ppr' | 'lsh' | 'spectral-sparsify';
  pprAlpha?: number;
  pprTopK?: number;
  lshTables?: number;
  lshBits?: number;
  spectralEpsilon?: number;
}

export interface ProofGatedOutput<T> {
  value: T;
  satisfied: boolean;
  attestationHex: string;
}

export interface ComplexityBound {
  opsUpperBound: number;
  memoryUpperBound: number;
  complexityClass: string;
}

export interface TrainingCertificate {
  totalSteps: number;
  violations: number;
  finalLoss: number;
  finalCoherence: number | null;
  attestationHex: string;
  invariantStats: InvariantStats[];
}
```

### Feature Parity Matrix

| Feature | Rust | WASM | Node.js |
|---------|------|------|---------|
| ProofGate<T> | Yes | Yes | Yes |
| Three-tier routing | Yes | Yes | Yes |
| Attestation chain | Yes | Yes | Yes |
| PPR-sampled attention | Yes | Yes | Yes |
| LSH spectral attention | Yes | Yes | Yes |
| Spectral sparsification | Yes | Yes | Yes |
| Hierarchical coarsening | Yes | No (1) | Yes |
| Memory-mapped processing | Yes | No (2) | Yes |
| VerifiedTrainer | Yes | No (3) | Yes |
| Robustness certification | Yes | No (3) | Yes |
| EWC continual learning | Yes | No (3) | Yes |
| Coherence (spectral) | Yes | No (4) | Yes |
| Coherence (basic) | Yes | Yes | Yes |

Notes:
1. Hierarchical coarsening uses `rayon` parallelism, unavailable in WASM
2. `mmap` is not available in WASM environments
3. Training is server-side only (see rationale above)
4. Spectral coherence uses `ndarray` with heavy numerics; excluded for size

### Build Pipeline

**WASM**:
```bash
cd crates/ruvector-graph-transformer-wasm
wasm-pack build --target web --release --out-dir ../../pkg/graph-transformer-wasm
# Verify size
ls -la ../../pkg/graph-transformer-wasm/*.wasm
```

**Node.js**:
```bash
cd crates/ruvector-graph-transformer-node
# NAPI-RS build for current platform
npx napi build --release --platform
# Cross-compile for CI (linux-x64-gnu, darwin-arm64, win32-x64-msvc)
npx napi build --release --target x86_64-unknown-linux-gnu
npx napi build --release --target aarch64-apple-darwin
npx napi build --release --target x86_64-pc-windows-msvc
```

### Testing Strategy

**WASM** (`wasm-bindgen-test`):
```rust
#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;
    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_graph_transformer_roundtrip() {
        let config = r#"{"proofGated": true, "attentionMechanism": "ppr"}"#;
        let gt = WasmGraphTransformer::new(config).unwrap();
        assert!(gt.coherence().is_ok());
    }

    #[wasm_bindgen_test]
    fn test_proof_chain_returns_uint8arrays() {
        // Verify attestation chain serialization
    }
}
```

**Node.js** (via `jest` or `vitest`):
```javascript
import { GraphTransformer, VerifiedTrainer } from '@ruvector/graph-transformer-node';

test('forward pass returns proof-gated output', () => {
  const gt = new GraphTransformer('{"proofGated": true, "attentionMechanism": "ppr"}');
  const result = gt.forward(batchJson);
  expect(result.satisfied).toBe(true);
  expect(result.attestationHex).toHaveLength(164); // 82 bytes = 164 hex chars
});

test('verified training produces certificate', () => {
  const trainer = new VerifiedTrainer(configJson);
  for (let i = 0; i < 10; i++) {
    trainer.step(loss, gradientsJson);
  }
  const cert = trainer.seal();
  expect(cert.totalSteps).toBe(10);
  expect(cert.violations).toBe(0);
});
```

### npm Package Names

- WASM: `@ruvector/graph-transformer-wasm`
- Node.js: `@ruvector/graph-transformer-node`

Both published under the `ruvnet` npm account (already authenticated per `CLAUDE.md`).

## Consequences

### Positive

- TypeScript/JavaScript developers get proof-gated graph transformers with zero Rust toolchain requirement
- WASM < 300 KB enables browser-side inference with proof verification
- Node.js bindings get full feature parity including verified training
- Consistent binding patterns with existing `-wasm` and `-node` crates reduce maintenance burden
- TypeScript types provide compile-time safety for JS consumers

### Negative

- WASM lacks training, hierarchical coarsening, and spectral coherence -- feature gap may confuse users
- Two binding crates double the CI build matrix
- NAPI-RS cross-compilation requires platform-specific CI runners (or cross-rs)
- Serialization overhead (JSON for config, `Uint8Array` for attestations) adds latency compared to native Rust

### Risks

- WASM size may exceed 300 KB if `ruvector-solver` brings in unexpected transitive dependencies. Mitigated by `default-features = false` and `wasm-pack --release` size verification in CI
- NAPI-RS version 2.16 may introduce breaking changes in minor releases. Mitigated by pinning to workspace version
- Browser `WebAssembly.Memory` limits (4 GB on 64-bit, 2 GB on 32-bit) may be hit for large graphs. Mitigated by streaming processing and the `certify_complexity` API that rejects oversized graphs before execution

## Implementation

1. Create `crates/ruvector-graph-transformer-wasm/` following the structure above
2. Create `crates/ruvector-graph-transformer-node/` following the structure above
3. Add both to `[workspace.members]` in root `Cargo.toml`
4. Implement Tier 1 (core) bindings first, test with `wasm-bindgen-test` and Node.js
5. Implement Tier 2 (attention) bindings
6. Implement Tier 3 (training) in Node.js only
7. CI: add `wasm-pack build` and `napi build` to GitHub Actions workflow
8. Publish to npm: `@ruvector/graph-transformer-wasm` and `@ruvector/graph-transformer-node`

## References

- ADR-046: Graph Transformer Unified Architecture (module structure, feature flags)
- ADR-047: Proof-Gated Mutation Protocol (`ProofGate<T>`, `ProofAttestation` serialization)
- ADR-048: Sublinear Graph Attention (attention API surface)
- ADR-049: Verified Training Pipeline (`VerifiedTrainer`, `TrainingCertificate`)
- `crates/ruvector-gnn-wasm/Cargo.toml`: WASM binding pattern (opt-level "z", panic "abort")
- `crates/ruvector-gnn-node/Cargo.toml`: NAPI-RS binding pattern (napi-build, cdylib)
- `crates/ruvector-verified-wasm/Cargo.toml`: Verified WASM binding pattern (serde-wasm-bindgen)
- `crates/ruvector-graph-wasm/Cargo.toml`: Graph WASM binding pattern
- Workspace `Cargo.toml`: `wasm-bindgen = "0.2"`, `napi = { version = "2.16" }`, `napi-derive = "2.16"`
