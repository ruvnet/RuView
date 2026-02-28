# Platform Bindings Specification - RuVector Attention

**Version:** 1.0.0
**Date:** 2025-11-30
**Status:** Implementation Ready

## Executive Summary

This document specifies comprehensive platform bindings for `ruvector-attention`, enabling deployment across Rust native, WebAssembly (browser and server), Node.js (via NAPI-RS), and providing CLI/SDK interfaces for maximum accessibility.

## 1. Platform Support Matrix

| Platform | Target | Status | Priority | Notes |
|----------|--------|--------|----------|-------|
| **Rust Native** | All Tier 1 targets | Primary | P0 | Core implementation |
| **WASM (Browser)** | wasm32-unknown-unknown | Full | P0 | Browser/Deno runtime |
| **WASM (Server)** | wasm32-wasi | Full | P1 | Server-side WASM |
| **Node.js 18 LTS** | x86_64, arm64 | Full | P0 | Long-term support |
| **Node.js 20 LTS** | x86_64, arm64 | Full | P0 | Long-term support |
| **Node.js 22** | x86_64, arm64 | Full | P1 | Current release |
| **Windows** | x86_64-pc-windows-msvc | Full | P0 | NAPI-RS bindings |
| **macOS Intel** | x86_64-apple-darwin | Full | P0 | NAPI-RS bindings |
| **macOS Apple Silicon** | aarch64-apple-darwin | Full | P0 | NAPI-RS bindings |
| **Linux x64** | x86_64-unknown-linux-gnu | Full | P0 | NAPI-RS (glibc) |
| **Linux x64 (musl)** | x86_64-unknown-linux-musl | Full | P1 | Alpine/static linking |
| **Linux ARM64** | aarch64-unknown-linux-gnu | Full | P1 | NAPI-RS (glibc) |
| **Linux ARM64 (musl)** | aarch64-unknown-linux-musl | Full | P2 | Alpine ARM |
| **Linux ARMv7** | armv7-unknown-linux-gnueabihf | Partial | P2 | Raspberry Pi |

### 1.1 Feature Matrix by Platform

| Feature | Rust | WASM | Node.js | CLI |
|---------|------|------|---------|-----|
| Scaled Dot-Product | ✅ | ✅ | ✅ | ✅ |
| Multi-Head Attention | ✅ | ✅ | ✅ | ✅ |
| Hyperbolic Attention | ✅ | ✅ | ✅ | ✅ |
| Linear Attention | ✅ | ✅ | ✅ | ✅ |
| Cross Attention | ✅ | ✅ | ✅ | ✅ |
| Self Attention | ✅ | ✅ | ✅ | ✅ |
| SIMD Optimizations | ✅ | ⚠️ | ✅ | ✅ |
| Async Processing | ✅ | ✅ | ✅ | N/A |
| Batch Operations | ✅ | ✅ | ✅ | ✅ |
| Streaming | ✅ | ⚠️ | ✅ | ✅ |

⚠️ = Requires experimental flags or limited support

---

## 2. WASM Bindings

### 2.1 Project Structure

```
crates/ruvector-attention-wasm/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs                 # Main WASM exports
│   ├── attention/
│   │   ├── mod.rs
│   │   ├── scaled_dot.rs      # ScaledDotProduct WASM wrapper
│   │   ├── multi_head.rs      # MultiHead WASM wrapper
│   │   ├── hyperbolic.rs      # Hyperbolic WASM wrapper
│   │   ├── linear.rs          # Linear WASM wrapper
│   │   └── cross.rs           # Cross attention WASM wrapper
│   ├── utils.rs               # WASM utilities (panic hook, logging)
│   ├── error.rs               # WASM error handling
│   ├── types.rs               # JS-compatible types
│   └── async_ops.rs           # Async operations for WASM
├── pkg/                       # wasm-pack build output (gitignored)
├── web/                       # Browser examples
│   ├── index.html
│   ├── demo.js
│   └── worker.js             # Web Worker example
├── node/                      # Node.js WASM examples
│   └── example.mjs
├── tests/
│   ├── web.rs                # wasm-bindgen-test
│   └── node.rs
├── benches/
│   └── wasm_bench.rs
└── examples/
    ├── browser_basic.html
    ├── browser_worker.html
    └── node_server.mjs
```

### 2.2 Cargo Configuration

```toml
# crates/ruvector-attention-wasm/Cargo.toml
[package]
name = "ruvector-attention-wasm"
version = "0.1.0"
authors = ["RuVector Team"]
edition = "2021"
license = "MIT OR Apache-2.0"
description = "WebAssembly bindings for RuVector attention mechanisms"
repository = "https://github.com/yourusername/ruvector"

[lib]
crate-type = ["cdylib", "rlib"]

[features]
default = ["console_error_panic_hook", "wee_alloc"]
simd = ["packed_simd"]
parallel = ["wasm-bindgen-rayon"]

[dependencies]
ruvector-attention = { path = "../ruvector-attention" }
wasm-bindgen = "0.2.92"
wasm-bindgen-futures = "0.4.42"
js-sys = "0.3.69"
web-sys = { version = "0.3.69", features = [
    "console",
    "Performance",
    "PerformanceTiming",
    "Window",
    "Worker",
] }
serde = { version = "1.0", features = ["derive"] }
serde-wasm-bindgen = "0.6"
console_error_panic_hook = { version = "0.1.7", optional = true }
wee_alloc = { version = "0.4.5", optional = true }
wasm-bindgen-rayon = { version = "1.2", optional = true }

[dev-dependencies]
wasm-bindgen-test = "0.3.42"
criterion = "0.5"

[profile.release]
# Optimize for size and speed
opt-level = "z"        # Optimize for size
lto = true             # Enable Link Time Optimization
codegen-units = 1      # Reduce parallel codegen for better optimization
panic = "abort"        # Remove panic formatting code
strip = true           # Strip symbols

[profile.release.package."*"]
opt-level = "z"
```

### 2.3 WASM API Design

#### 2.3.1 Core Library (lib.rs)

```rust
// crates/ruvector-attention-wasm/src/lib.rs
use wasm_bindgen::prelude::*;

mod attention;
mod error;
mod types;
mod utils;

pub use attention::*;
pub use error::WasmError;
pub use types::*;

// Initialize WASM module
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    #[cfg(feature = "wee_alloc")]
    {
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
    }
}

// Version info
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
```

#### 2.3.2 Scaled Dot-Product Attention

```rust
// crates/ruvector-attention-wasm/src/attention/scaled_dot.rs
use wasm_bindgen::prelude::*;
use js_sys::{Float32Array, Array};
use ruvector_attention::ScaledDotProduct;
use crate::error::WasmError;

#[wasm_bindgen]
pub struct WasmScaledDotProduct {
    inner: ScaledDotProduct,
}

#[wasm_bindgen]
impl WasmScaledDotProduct {
    /// Create a new scaled dot-product attention layer
    #[wasm_bindgen(constructor)]
    pub fn new(dim: usize) -> Result<WasmScaledDotProduct, WasmError> {
        Ok(Self {
            inner: ScaledDotProduct::new(dim),
        })
    }

    /// Forward pass with single query
    #[wasm_bindgen]
    pub fn forward(
        &self,
        query: &[f32],
        keys: Float32Array,
        values: Float32Array,
        num_neighbors: usize,
    ) -> Result<Float32Array, WasmError> {
        let keys_vec: Vec<f32> = keys.to_vec();
        let values_vec: Vec<f32> = values.to_vec();

        let result = self.inner.forward(
            query,
            &keys_vec,
            &values_vec,
            num_neighbors,
        )?;

        Ok(Float32Array::from(&result[..]))
    }

    /// Forward pass with batched queries
    #[wasm_bindgen]
    pub fn forward_batch(
        &self,
        queries: Array,
        keys: Float32Array,
        values: Float32Array,
        num_neighbors: usize,
    ) -> Result<Array, WasmError> {
        let keys_vec: Vec<f32> = keys.to_vec();
        let values_vec: Vec<f32> = values.to_vec();

        let results = Array::new();
        for i in 0..queries.length() {
            let query_arr = Float32Array::from(queries.get(i));
            let query_vec: Vec<f32> = query_arr.to_vec();

            let result = self.inner.forward(
                &query_vec,
                &keys_vec,
                &values_vec,
                num_neighbors,
            )?;

            results.push(&Float32Array::from(&result[..]));
        }

        Ok(results)
    }

    /// Async forward pass (runs on web worker or async runtime)
    #[wasm_bindgen]
    pub async fn forward_async(
        &self,
        query: Vec<f32>,
        keys: Float32Array,
        values: Float32Array,
        num_neighbors: usize,
    ) -> Result<Float32Array, WasmError> {
        let keys_vec: Vec<f32> = keys.to_vec();
        let values_vec: Vec<f32> = values.to_vec();

        // Use wasm-bindgen-futures for async execution
        let result = wasm_bindgen_futures::spawn_local(async move {
            self.inner.forward(&query, &keys_vec, &values_vec, num_neighbors)
        })
        .await?;

        Ok(Float32Array::from(&result[..]))
    }
}
```

#### 2.3.3 Multi-Head Attention

```rust
// crates/ruvector-attention-wasm/src/attention/multi_head.rs
use wasm_bindgen::prelude::*;
use js_sys::Float32Array;
use ruvector_attention::MultiHeadAttention;
use crate::error::WasmError;

#[wasm_bindgen]
pub struct WasmMultiHeadAttention {
    inner: MultiHeadAttention,
}

#[wasm_bindgen]
impl WasmMultiHeadAttention {
    #[wasm_bindgen(constructor)]
    pub fn new(
        num_heads: usize,
        hidden_dim: usize,
        dropout: f32,
    ) -> Result<WasmMultiHeadAttention, WasmError> {
        Ok(Self {
            inner: MultiHeadAttention::new(num_heads, hidden_dim, dropout)?,
        })
    }

    #[wasm_bindgen]
    pub fn forward(
        &self,
        query: &[f32],
        keys: Float32Array,
        values: Float32Array,
        mask: Option<Vec<bool>>,
    ) -> Result<Float32Array, WasmError> {
        let keys_vec: Vec<f32> = keys.to_vec();
        let values_vec: Vec<f32> = values.to_vec();

        let result = self.inner.forward(
            query,
            &keys_vec,
            &values_vec,
            mask.as_deref(),
        )?;

        Ok(Float32Array::from(&result[..]))
    }

    #[wasm_bindgen(getter)]
    pub fn num_heads(&self) -> usize {
        self.inner.num_heads()
    }

    #[wasm_bindgen(getter)]
    pub fn hidden_dim(&self) -> usize {
        self.inner.hidden_dim()
    }
}
```

#### 2.3.4 Hyperbolic Attention

```rust
// crates/ruvector-attention-wasm/src/attention/hyperbolic.rs
use wasm_bindgen::prelude::*;
use js_sys::Float32Array;
use ruvector_attention::HyperbolicAttention;
use crate::error::WasmError;

#[wasm_bindgen]
pub struct WasmHyperbolicAttention {
    inner: HyperbolicAttention,
}

#[wasm_bindgen]
impl WasmHyperbolicAttention {
    #[wasm_bindgen(constructor)]
    pub fn new(curvature: f32) -> Result<WasmHyperbolicAttention, WasmError> {
        Ok(Self {
            inner: HyperbolicAttention::new(curvature),
        })
    }

    #[wasm_bindgen]
    pub fn forward(
        &self,
        query: &[f32],
        keys: Float32Array,
        values: Float32Array,
    ) -> Result<Float32Array, WasmError> {
        let keys_vec: Vec<f32> = keys.to_vec();
        let values_vec: Vec<f32> = values.to_vec();

        let result = self.inner.forward(query, &keys_vec, &values_vec)?;

        Ok(Float32Array::from(&result[..]))
    }

    /// Compute Poincaré distance between two points
    #[wasm_bindgen]
    pub fn poincare_distance(&self, x: &[f32], y: &[f32]) -> Result<f32, WasmError> {
        self.inner.poincare_distance(x, y)
    }

    /// Project Euclidean point to Poincaré ball
    #[wasm_bindgen]
    pub fn to_poincare(&self, x: &[f32]) -> Result<Float32Array, WasmError> {
        let result = self.inner.to_poincare(x)?;
        Ok(Float32Array::from(&result[..]))
    }

    #[wasm_bindgen(getter)]
    pub fn curvature(&self) -> f32 {
        self.inner.curvature()
    }
}
```

### 2.4 Build Scripts and Configuration

#### 2.4.1 Build Script

```bash
#!/bin/bash
# scripts/build-wasm.sh

set -e

echo "Building RuVector Attention WASM..."

# Clean previous builds
rm -rf pkg/

# Build for web (browser)
echo "Building for web target..."
wasm-pack build \
    --target web \
    --out-dir pkg/web \
    --release \
    crates/ruvector-attention-wasm

# Build for Node.js
echo "Building for Node.js target..."
wasm-pack build \
    --target nodejs \
    --out-dir pkg/nodejs \
    --release \
    crates/ruvector-attention-wasm

# Build for bundlers (webpack, vite, etc.)
echo "Building for bundler target..."
wasm-pack build \
    --target bundler \
    --out-dir pkg/bundler \
    --release \
    crates/ruvector-attention-wasm

# Optional: Build with SIMD (requires nightly + flags)
if [ "$BUILD_SIMD" = "1" ]; then
    echo "Building SIMD version (requires nightly)..."
    RUSTFLAGS="-C target-feature=+simd128" \
    wasm-pack build \
        --target web \
        --out-dir pkg/web-simd \
        --release \
        -- --features simd \
        crates/ruvector-attention-wasm
fi

echo "WASM build complete!"
echo "Outputs:"
echo "  - Web:     pkg/web/"
echo "  - Node.js: pkg/nodejs/"
echo "  - Bundler: pkg/bundler/"
```

#### 2.4.2 Package.json (for NPM publishing)

```json
{
  "name": "ruvector-attention-wasm",
  "version": "0.1.0",
  "description": "WebAssembly bindings for RuVector attention mechanisms",
  "main": "pkg/nodejs/ruvector_attention_wasm.js",
  "module": "pkg/web/ruvector_attention_wasm.js",
  "types": "pkg/web/ruvector_attention_wasm.d.ts",
  "files": [
    "pkg/**/*"
  ],
  "scripts": {
    "build": "./scripts/build-wasm.sh",
    "test": "wasm-pack test --headless --chrome --firefox",
    "test:node": "wasm-pack test --node"
  },
  "keywords": [
    "wasm",
    "attention",
    "machine-learning",
    "rust",
    "webassembly"
  ],
  "license": "MIT OR Apache-2.0",
  "repository": {
    "type": "git",
    "url": "https://github.com/yourusername/ruvector"
  }
}
```

### 2.5 TypeScript Definitions (Auto-generated)

```typescript
// pkg/web/ruvector_attention_wasm.d.ts (generated by wasm-bindgen)
export function init(): void;
export function version(): string;

export class WasmScaledDotProduct {
    constructor(dim: number);
    free(): void;
    forward(
        query: Float32Array,
        keys: Float32Array,
        values: Float32Array,
        numNeighbors: number
    ): Float32Array;
    forwardBatch(
        queries: Float32Array[],
        keys: Float32Array,
        values: Float32Array,
        numNeighbors: number
    ): Float32Array[];
    forwardAsync(
        query: Float32Array,
        keys: Float32Array,
        values: Float32Array,
        numNeighbors: number
    ): Promise<Float32Array>;
}

export class WasmMultiHeadAttention {
    constructor(numHeads: number, hiddenDim: number, dropout: number);
    free(): void;
    forward(
        query: Float32Array,
        keys: Float32Array,
        values: Float32Array,
        mask?: boolean[]
    ): Float32Array;
    readonly numHeads: number;
    readonly hiddenDim: number;
}

export class WasmHyperbolicAttention {
    constructor(curvature: number);
    free(): void;
    forward(
        query: Float32Array,
        keys: Float32Array,
        values: Float32Array
    ): Float32Array;
    poincareDistance(x: Float32Array, y: Float32Array): number;
    toPoincare(x: Float32Array): Float32Array;
    readonly curvature: number;
}
```

---

## 3. NAPI-RS Bindings (Node.js)

### 3.1 Project Structure

```
crates/ruvector-attention-node/
├── Cargo.toml
├── build.rs
├── package.json
├── index.js                   # JS entry point
├── index.d.ts                 # TypeScript definitions
├── README.md
├── src/
│   ├── lib.rs                 # NAPI exports
│   ├── attention/
│   │   ├── mod.rs
│   │   ├── scaled_dot.rs
│   │   ├── multi_head.rs
│   │   ├── hyperbolic.rs
│   │   ├── linear.rs
│   │   └── cross.rs
│   ├── async_ops.rs           # Tokio async operations
│   ├── error.rs               # NAPI error handling
│   ├── buffer.rs              # Buffer conversions
│   └── types.rs               # Type conversions
├── npm/                       # Platform-specific packages
│   ├── darwin-arm64/
│   │   ├── package.json
│   │   └── README.md
│   ├── darwin-x64/
│   ├── linux-arm64-gnu/
│   ├── linux-arm64-musl/
│   ├── linux-x64-gnu/
│   ├── linux-x64-musl/
│   ├── win32-arm64-msvc/
│   └── win32-x64-msvc/
├── __tests__/
│   ├── basic.test.ts
│   ├── async.test.ts
│   └── performance.test.ts
└── examples/
    ├── basic.mjs
    ├── async.mjs
    └── streaming.mjs
```

### 3.2 Cargo Configuration

```toml
# crates/ruvector-attention-node/Cargo.toml
[package]
name = "ruvector-attention-node"
version = "0.1.0"
authors = ["RuVector Team"]
edition = "2021"
license = "MIT OR Apache-2.0"
description = "Node.js bindings for RuVector attention mechanisms via NAPI-RS"

[lib]
crate-type = ["cdylib"]

[dependencies]
ruvector-attention = { path = "../ruvector-attention" }
napi = { version = "2.16", features = ["async", "napi8", "tokio_rt"] }
napi-derive = "2.16"
tokio = { version = "1.37", features = ["rt-multi-thread"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[build-dependencies]
napi-build = "2.1"

[profile.release]
lto = true
codegen-units = 1
opt-level = 3
strip = true
```

### 3.3 NAPI API Design

#### 3.3.1 Core Library

```rust
// crates/ruvector-attention-node/src/lib.rs
#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

mod attention;
mod async_ops;
mod buffer;
mod error;
mod types;

pub use attention::*;
pub use error::NapiError;

#[napi]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[napi]
pub fn get_supported_features() -> Vec<String> {
    vec![
        "scaled-dot-product".to_string(),
        "multi-head".to_string(),
        "hyperbolic".to_string(),
        "linear".to_string(),
        "cross-attention".to_string(),
        "self-attention".to_string(),
    ]
}
```

#### 3.3.2 Scaled Dot-Product Attention

```rust
// crates/ruvector-attention-node/src/attention/scaled_dot.rs
use napi::bindgen_prelude::*;
use napi_derive::napi;
use ruvector_attention::ScaledDotProduct as CoreScaledDotProduct;

#[napi]
pub struct ScaledDotProductAttention {
    inner: CoreScaledDotProduct,
}

#[napi]
impl ScaledDotProductAttention {
    #[napi(constructor)]
    pub fn new(dim: u32) -> Result<Self> {
        Ok(Self {
            inner: CoreScaledDotProduct::new(dim as usize),
        })
    }

    /// Synchronous forward pass
    #[napi]
    pub fn forward(
        &self,
        query: Float32Array,
        keys: Float32Array,
        values: Float32Array,
        num_neighbors: u32,
    ) -> Result<Float32Array> {
        let query_vec = query.to_vec();
        let keys_vec = keys.to_vec();
        let values_vec = values.to_vec();

        let result = self.inner
            .forward(&query_vec, &keys_vec, &values_vec, num_neighbors as usize)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Float32Array::new(result))
    }

    /// Asynchronous forward pass (non-blocking)
    #[napi]
    pub async fn forward_async(
        &self,
        query: Float32Array,
        keys: Float32Array,
        values: Float32Array,
        num_neighbors: u32,
    ) -> Result<Float32Array> {
        let query_vec = query.to_vec();
        let keys_vec = keys.to_vec();
        let values_vec = values.to_vec();
        let inner = self.inner.clone();

        tokio::task::spawn_blocking(move || {
            inner.forward(&query_vec, &keys_vec, &values_vec, num_neighbors as usize)
        })
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?
        .map_err(|e| Error::from_reason(e.to_string()))
        .map(Float32Array::new)
    }

    /// Batch forward pass
    #[napi]
    pub fn forward_batch(
        &self,
        queries: Vec<Float32Array>,
        keys: Float32Array,
        values: Float32Array,
        num_neighbors: u32,
    ) -> Result<Vec<Float32Array>> {
        let keys_vec = keys.to_vec();
        let values_vec = values.to_vec();

        queries
            .into_iter()
            .map(|q| {
                let query_vec = q.to_vec();
                self.inner
                    .forward(&query_vec, &keys_vec, &values_vec, num_neighbors as usize)
                    .map(Float32Array::new)
                    .map_err(|e| Error::from_reason(e.to_string()))
            })
            .collect()
    }

    /// Async batch forward pass
    #[napi]
    pub async fn forward_batch_async(
        &self,
        queries: Vec<Float32Array>,
        keys: Float32Array,
        values: Float32Array,
        num_neighbors: u32,
    ) -> Result<Vec<Float32Array>> {
        let keys_vec = keys.to_vec();
        let values_vec = values.to_vec();
        let inner = self.inner.clone();

        let query_vecs: Vec<Vec<f32>> = queries.into_iter()
            .map(|q| q.to_vec())
            .collect();

        tokio::task::spawn_blocking(move || {
            query_vecs.into_iter()
                .map(|q| {
                    inner.forward(&q, &keys_vec, &values_vec, num_neighbors as usize)
                        .map(Float32Array::new)
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?
        .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi(getter)]
    pub fn dim(&self) -> u32 {
        self.inner.dim() as u32
    }
}
```

#### 3.3.3 Multi-Head Attention

```rust
// crates/ruvector-attention-node/src/attention/multi_head.rs
use napi::bindgen_prelude::*;
use napi_derive::napi;
use ruvector_attention::MultiHeadAttention as CoreMultiHeadAttention;

#[napi(object)]
pub struct AttentionConfig {
    pub num_heads: u32,
    pub hidden_dim: u32,
    pub dropout: f64,
}

#[napi]
pub struct MultiHeadAttention {
    inner: CoreMultiHeadAttention,
}

#[napi]
impl MultiHeadAttention {
    #[napi(constructor)]
    pub fn new(config: AttentionConfig) -> Result<Self> {
        let inner = CoreMultiHeadAttention::new(
            config.num_heads as usize,
            config.hidden_dim as usize,
            config.dropout as f32,
        )
        .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Self { inner })
    }

    #[napi(factory)]
    pub fn from_params(num_heads: u32, hidden_dim: u32, dropout: Option<f64>) -> Result<Self> {
        Self::new(AttentionConfig {
            num_heads,
            hidden_dim,
            dropout: dropout.unwrap_or(0.0),
        })
    }

    #[napi]
    pub fn forward(
        &self,
        query: Float32Array,
        keys: Float32Array,
        values: Float32Array,
        mask: Option<Vec<bool>>,
    ) -> Result<Float32Array> {
        let query_vec = query.to_vec();
        let keys_vec = keys.to_vec();
        let values_vec = values.to_vec();

        let result = self.inner
            .forward(&query_vec, &keys_vec, &values_vec, mask.as_deref())
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Float32Array::new(result))
    }

    #[napi]
    pub async fn forward_async(
        &self,
        query: Float32Array,
        keys: Float32Array,
        values: Float32Array,
        mask: Option<Vec<bool>>,
    ) -> Result<Float32Array> {
        let query_vec = query.to_vec();
        let keys_vec = keys.to_vec();
        let values_vec = values.to_vec();
        let inner = self.inner.clone();

        tokio::task::spawn_blocking(move || {
            inner.forward(&query_vec, &keys_vec, &values_vec, mask.as_deref())
        })
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?
        .map_err(|e| Error::from_reason(e.to_string()))
        .map(Float32Array::new)
    }

    #[napi(getter)]
    pub fn num_heads(&self) -> u32 {
        self.inner.num_heads() as u32
    }

    #[napi(getter)]
    pub fn hidden_dim(&self) -> u32 {
        self.inner.hidden_dim() as u32
    }

    #[napi(getter)]
    pub fn head_dim(&self) -> u32 {
        (self.inner.hidden_dim() / self.inner.num_heads()) as u32
    }
}
```

#### 3.3.4 Hyperbolic Attention

```rust
// crates/ruvector-attention-node/src/attention/hyperbolic.rs
use napi::bindgen_prelude::*;
use napi_derive::napi;
use ruvector_attention::HyperbolicAttention as CoreHyperbolicAttention;

#[napi]
pub struct HyperbolicAttention {
    inner: CoreHyperbolicAttention,
}

#[napi]
impl HyperbolicAttention {
    #[napi(constructor)]
    pub fn new(curvature: f64) -> Result<Self> {
        Ok(Self {
            inner: CoreHyperbolicAttention::new(curvature as f32),
        })
    }

    #[napi]
    pub fn forward(
        &self,
        query: Float32Array,
        keys: Float32Array,
        values: Float32Array,
    ) -> Result<Float32Array> {
        let query_vec = query.to_vec();
        let keys_vec = keys.to_vec();
        let values_vec = values.to_vec();

        let result = self.inner
            .forward(&query_vec, &keys_vec, &values_vec)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Float32Array::new(result))
    }

    #[napi]
    pub async fn forward_async(
        &self,
        query: Float32Array,
        keys: Float32Array,
        values: Float32Array,
    ) -> Result<Float32Array> {
        let query_vec = query.to_vec();
        let keys_vec = keys.to_vec();
        let values_vec = values.to_vec();
        let inner = self.inner.clone();

        tokio::task::spawn_blocking(move || {
            inner.forward(&query_vec, &keys_vec, &values_vec)
        })
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?
        .map_err(|e| Error::from_reason(e.to_string()))
        .map(Float32Array::new)
    }

    #[napi]
    pub fn poincare_distance(&self, x: Float32Array, y: Float32Array) -> Result<f64> {
        let x_vec = x.to_vec();
        let y_vec = y.to_vec();

        self.inner
            .poincare_distance(&x_vec, &y_vec)
            .map(|d| d as f64)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn to_poincare(&self, x: Float32Array) -> Result<Float32Array> {
        let x_vec = x.to_vec();

        self.inner
            .to_poincare(&x_vec)
            .map(Float32Array::new)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn from_poincare(&self, x: Float32Array) -> Result<Float32Array> {
        let x_vec = x.to_vec();

        self.inner
            .from_poincare(&x_vec)
            .map(Float32Array::new)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi(getter)]
    pub fn curvature(&self) -> f64 {
        self.inner.curvature() as f64
    }
}
```

### 3.4 Package Configuration

#### 3.4.1 package.json

```json
{
  "name": "ruvector-attention",
  "version": "0.1.0",
  "description": "High-performance attention mechanisms for Node.js via Rust/NAPI-RS",
  "main": "index.js",
  "types": "index.d.ts",
  "keywords": [
    "attention",
    "machine-learning",
    "rust",
    "napi",
    "native",
    "performance"
  ],
  "license": "MIT OR Apache-2.0",
  "author": "RuVector Team",
  "repository": {
    "type": "git",
    "url": "https://github.com/yourusername/ruvector"
  },
  "engines": {
    "node": ">= 18"
  },
  "napi": {
    "name": "ruvector-attention",
    "triples": {
      "defaults": true,
      "additional": [
        "aarch64-apple-darwin",
        "aarch64-unknown-linux-gnu",
        "aarch64-unknown-linux-musl",
        "armv7-unknown-linux-gnueabihf",
        "x86_64-unknown-linux-musl",
        "aarch64-pc-windows-msvc"
      ]
    }
  },
  "scripts": {
    "artifacts": "napi artifacts",
    "build": "napi build --platform --release",
    "build:debug": "napi build --platform",
    "prepublishOnly": "napi prepublish -t npm",
    "test": "jest",
    "test:coverage": "jest --coverage",
    "version": "napi version",
    "bench": "node benches/benchmark.mjs"
  },
  "devDependencies": {
    "@napi-rs/cli": "^2.18.0",
    "@types/node": "^20.11.0",
    "jest": "^29.7.0",
    "typescript": "^5.3.3"
  },
  "optionalDependencies": {
    "ruvector-attention-darwin-arm64": "0.1.0",
    "ruvector-attention-darwin-x64": "0.1.0",
    "ruvector-attention-linux-arm64-gnu": "0.1.0",
    "ruvector-attention-linux-arm64-musl": "0.1.0",
    "ruvector-attention-linux-x64-gnu": "0.1.0",
    "ruvector-attention-linux-x64-musl": "0.1.0",
    "ruvector-attention-win32-arm64-msvc": "0.1.0",
    "ruvector-attention-win32-x64-msvc": "0.1.0"
  }
}
```

#### 3.4.2 TypeScript Definitions

```typescript
// index.d.ts
export function getVersion(): string;
export function getSupportedFeatures(): string[];

export interface AttentionConfig {
  numHeads: number;
  hiddenDim: number;
  dropout: number;
}

export class ScaledDotProductAttention {
  constructor(dim: number);

  forward(
    query: Float32Array,
    keys: Float32Array,
    values: Float32Array,
    numNeighbors: number
  ): Float32Array;

  forwardAsync(
    query: Float32Array,
    keys: Float32Array,
    values: Float32Array,
    numNeighbors: number
  ): Promise<Float32Array>;

  forwardBatch(
    queries: Float32Array[],
    keys: Float32Array,
    values: Float32Array,
    numNeighbors: number
  ): Float32Array[];

  forwardBatchAsync(
    queries: Float32Array[],
    keys: Float32Array,
    values: Float32Array,
    numNeighbors: number
  ): Promise<Float32Array[]>;

  readonly dim: number;
}

export class MultiHeadAttention {
  constructor(config: AttentionConfig);

  static fromParams(
    numHeads: number,
    hiddenDim: number,
    dropout?: number
  ): MultiHeadAttention;

  forward(
    query: Float32Array,
    keys: Float32Array,
    values: Float32Array,
    mask?: boolean[]
  ): Float32Array;

  forwardAsync(
    query: Float32Array,
    keys: Float32Array,
    values: Float32Array,
    mask?: boolean[]
  ): Promise<Float32Array>;

  readonly numHeads: number;
  readonly hiddenDim: number;
  readonly headDim: number;
}

export class HyperbolicAttention {
  constructor(curvature: number);

  forward(
    query: Float32Array,
    keys: Float32Array,
    values: Float32Array
  ): Float32Array;

  forwardAsync(
    query: Float32Array,
    keys: Float32Array,
    values: Float32Array
  ): Promise<Float32Array>;

  poincareDistance(x: Float32Array, y: Float32Array): number;
  toPoincare(x: Float32Array): Float32Array;
  fromPoincare(x: Float32Array): Float32Array;

  readonly curvature: number;
}

// Export all attention types
export { ScaledDotProductAttention as ScaledDotProduct };
export { MultiHeadAttention as MultiHead };
export { HyperbolicAttention as Hyperbolic };
```

### 3.5 Build and Deployment

#### 3.5.1 GitHub Actions Workflow

```yaml
# .github/workflows/napi.yml
name: NAPI Build and Release

on:
  push:
    branches: [main]
    tags: ['v*']
  pull_request:

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        settings:
          - host: macos-latest
            target: x86_64-apple-darwin
            build: pnpm build --target x86_64-apple-darwin
          - host: macos-latest
            target: aarch64-apple-darwin
            build: pnpm build --target aarch64-apple-darwin
          - host: windows-latest
            target: x86_64-pc-windows-msvc
            build: pnpm build --target x86_64-pc-windows-msvc
          - host: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            build: pnpm build --target x86_64-unknown-linux-gnu
          - host: ubuntu-latest
            target: x86_64-unknown-linux-musl
            build: pnpm build --target x86_64-unknown-linux-musl
          - host: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            build: pnpm build --target aarch64-unknown-linux-gnu

    name: Build ${{ matrix.settings.target }}
    runs-on: ${{ matrix.settings.host }}

    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v2
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'pnpm'

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.settings.target }}

      - name: Build
        run: ${{ matrix.settings.build }}

      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: bindings-${{ matrix.settings.target }}
          path: crates/ruvector-attention-node/*.node
```

---

## 4. CLI Interface

### 4.1 Project Structure

```
crates/ruvector-attention-cli/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs                # CLI entry point
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── compute.rs         # Compute attention
│   │   ├── benchmark.rs       # Benchmarking
│   │   ├── convert.rs         # Model conversion
│   │   ├── serve.rs           # HTTP server
│   │   └── repl.rs            # Interactive REPL
│   ├── config.rs              # Configuration management
│   ├── format.rs              # Input/output formats
│   ├── server/
│   │   ├── mod.rs
│   │   ├── handlers.rs
│   │   └── middleware.rs
│   └── utils.rs
├── tests/
│   ├── integration.rs
│   └── cli.rs
└── examples/
    ├── config.toml
    └── sample_data/
```

### 4.2 Cargo Configuration

```toml
# crates/ruvector-attention-cli/Cargo.toml
[package]
name = "ruvector-attention-cli"
version = "0.1.0"
authors = ["RuVector Team"]
edition = "2021"
license = "MIT OR Apache-2.0"
description = "CLI for RuVector attention mechanisms"

[[bin]]
name = "ruvector-attention"
path = "src/main.rs"

[dependencies]
ruvector-attention = { path = "../ruvector-attention" }
clap = { version = "4.5", features = ["derive", "cargo"] }
tokio = { version = "1.37", features = ["full"] }
axum = "0.7"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
indicatif = "0.17"
comfy-table = "7.1"
colored = "2.1"
rustyline = "14.0"

[dev-dependencies]
assert_cmd = "2.0"
predicates = "3.1"
tempfile = "3.10"

[profile.release]
lto = true
codegen-units = 1
opt-level = 3
strip = true
```

### 4.3 CLI Design

#### 4.3.1 Main CLI Structure

```rust
// src/main.rs
use clap::{Parser, Subcommand};
use anyhow::Result;

mod commands;
mod config;
mod format;
mod server;
mod utils;

#[derive(Parser)]
#[command(name = "ruvector-attention")]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Configuration file
    #[arg(short, long, global = true)]
    config: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Compute attention for given inputs
    Compute(commands::compute::ComputeArgs),

    /// Run benchmarks
    Benchmark(commands::benchmark::BenchmarkArgs),

    /// Convert between model formats
    Convert(commands::convert::ConvertArgs),

    /// Start HTTP server
    Serve(commands::serve::ServeArgs),

    /// Interactive REPL
    Repl(commands::repl::ReplArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .init();

    // Execute command
    match cli.command {
        Commands::Compute(args) => commands::compute::run(args).await,
        Commands::Benchmark(args) => commands::benchmark::run(args).await,
        Commands::Convert(args) => commands::convert::run(args).await,
        Commands::Serve(args) => commands::serve::run(args).await,
        Commands::Repl(args) => commands::repl::run(args).await,
    }
}
```

#### 4.3.2 Compute Command

```rust
// src/commands/compute.rs
use clap::Args;
use anyhow::{Result, Context};
use ruvector_attention::*;
use std::path::PathBuf;

#[derive(Args)]
pub struct ComputeArgs {
    /// Attention type (scaled-dot-product, multi-head, hyperbolic, linear)
    #[arg(short, long)]
    attention_type: String,

    /// Query vector file (binary f32)
    #[arg(short, long)]
    query: PathBuf,

    /// Keys file (binary f32)
    #[arg(short, long)]
    keys: PathBuf,

    /// Values file (binary f32)
    #[arg(short, long)]
    values: PathBuf,

    /// Output file
    #[arg(short, long)]
    output: PathBuf,

    /// Number of neighbors (for k-NN attention)
    #[arg(long, default_value = "100")]
    neighbors: usize,

    /// Number of heads (for multi-head attention)
    #[arg(long, default_value = "8")]
    num_heads: usize,

    /// Hidden dimension
    #[arg(long, default_value = "128")]
    hidden_dim: usize,
}

pub async fn run(args: ComputeArgs) -> Result<()> {
    println!("Computing {} attention...", args.attention_type);

    // Load inputs
    let query = load_f32_binary(&args.query)
        .context("Failed to load query")?;
    let keys = load_f32_binary(&args.keys)
        .context("Failed to load keys")?;
    let values = load_f32_binary(&args.values)
        .context("Failed to load values")?;

    // Compute attention
    let result = match args.attention_type.as_str() {
        "scaled-dot-product" => {
            let attention = ScaledDotProduct::new(args.hidden_dim);
            attention.forward(&query, &keys, &values, args.neighbors)?
        },
        "multi-head" => {
            let attention = MultiHeadAttention::new(
                args.num_heads,
                args.hidden_dim,
                0.0,
            )?;
            attention.forward(&query, &keys, &values, None)?
        },
        "hyperbolic" => {
            let attention = HyperbolicAttention::new(1.0);
            attention.forward(&query, &keys, &values)?
        },
        _ => anyhow::bail!("Unknown attention type: {}", args.attention_type),
    };

    // Save output
    save_f32_binary(&args.output, &result)
        .context("Failed to save output")?;

    println!("✓ Attention computed successfully");
    println!("  Output shape: {}", result.len());
    println!("  Saved to: {}", args.output.display());

    Ok(())
}

fn load_f32_binary(path: &PathBuf) -> Result<Vec<f32>> {
    let bytes = std::fs::read(path)?;
    let floats: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    Ok(floats)
}

fn save_f32_binary(path: &PathBuf, data: &[f32]) -> Result<()> {
    let bytes: Vec<u8> = data.iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    std::fs::write(path, bytes)?;
    Ok(())
}
```

#### 4.3.3 Benchmark Command

```rust
// src/commands/benchmark.rs
use clap::Args;
use anyhow::Result;
use ruvector_attention::*;
use std::time::Instant;
use comfy_table::{Table, presets::UTF8_FULL};
use colored::Colorize;

#[derive(Args)]
pub struct BenchmarkArgs {
    /// Attention types to benchmark (comma-separated)
    #[arg(long, default_value = "scaled-dot-product,multi-head,hyperbolic")]
    types: String,

    /// Dimensions to test (comma-separated)
    #[arg(long, default_value = "128,256,512")]
    dims: String,

    /// Number of neighbors to test
    #[arg(long, default_value = "100,500,1000")]
    neighbors: String,

    /// Number of iterations
    #[arg(long, default_value = "1000")]
    iterations: usize,

    /// Output file for results (JSON)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

pub async fn run(args: BenchmarkArgs) -> Result<()> {
    println!("{}", "Running Benchmarks...".bold().green());

    let types: Vec<&str> = args.types.split(',').collect();
    let dims: Vec<usize> = args.dims.split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    let neighbors: Vec<usize> = args.neighbors.split(',')
        .filter_map(|s| s.parse().ok())
        .collect();

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Type", "Dim", "Neighbors", "Avg Time (ms)", "Throughput (ops/s)"]);

    for attention_type in &types {
        for &dim in &dims {
            for &k in &neighbors {
                let avg_time = benchmark_attention(
                    attention_type,
                    dim,
                    k,
                    args.iterations,
                )?;

                let throughput = 1000.0 / avg_time;

                table.add_row(vec![
                    attention_type.to_string(),
                    dim.to_string(),
                    k.to_string(),
                    format!("{:.3}", avg_time),
                    format!("{:.0}", throughput),
                ]);
            }
        }
    }

    println!("\n{}", table);

    Ok(())
}

fn benchmark_attention(
    attention_type: &str,
    dim: usize,
    num_neighbors: usize,
    iterations: usize,
) -> Result<f64> {
    // Generate random data
    let query: Vec<f32> = (0..dim).map(|_| rand::random()).collect();
    let keys: Vec<f32> = (0..dim * num_neighbors).map(|_| rand::random()).collect();
    let values: Vec<f32> = (0..dim * num_neighbors).map(|_| rand::random()).collect();

    let start = Instant::now();

    for _ in 0..iterations {
        match attention_type {
            "scaled-dot-product" => {
                let attention = ScaledDotProduct::new(dim);
                let _ = attention.forward(&query, &keys, &values, num_neighbors)?;
            },
            "multi-head" => {
                let attention = MultiHeadAttention::new(8, dim, 0.0)?;
                let _ = attention.forward(&query, &keys, &values, None)?;
            },
            "hyperbolic" => {
                let attention = HyperbolicAttention::new(1.0);
                let _ = attention.forward(&query, &keys, &values)?;
            },
            _ => {},
        }
    }

    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_secs_f64() * 1000.0 / iterations as f64;

    Ok(avg_ms)
}
```

#### 4.3.4 Server Command

```rust
// src/commands/serve.rs
use clap::Args;
use anyhow::Result;
use axum::{
    routing::{get, post},
    Router, Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Args)]
pub struct ServeArgs {
    /// Server host
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// Server port
    #[arg(short, long, default_value = "8080")]
    port: u16,

    /// Maximum batch size
    #[arg(long, default_value = "256")]
    max_batch_size: usize,
}

#[derive(Deserialize)]
struct AttentionRequest {
    attention_type: String,
    query: Vec<f32>,
    keys: Vec<f32>,
    values: Vec<f32>,
    num_neighbors: Option<usize>,
    num_heads: Option<usize>,
    hidden_dim: Option<usize>,
}

#[derive(Serialize)]
struct AttentionResponse {
    result: Vec<f32>,
    computation_time_ms: f64,
}

pub async fn run(args: ServeArgs) -> Result<()> {
    let app = Router::new()
        .route("/", get(health_check))
        .route("/attention", post(compute_attention))
        .route("/health", get(health_check));

    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

    println!("🚀 Server listening on http://{}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}

async fn compute_attention(
    Json(req): Json<AttentionRequest>,
) -> Result<Json<AttentionResponse>, StatusCode> {
    let start = std::time::Instant::now();

    let result = match req.attention_type.as_str() {
        "scaled-dot-product" => {
            let dim = req.hidden_dim.unwrap_or(128);
            let k = req.num_neighbors.unwrap_or(100);
            let attention = ScaledDotProduct::new(dim);
            attention.forward(&req.query, &req.keys, &req.values, k)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        },
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let elapsed = start.elapsed().as_secs_f64() * 1000.0;

    Ok(Json(AttentionResponse {
        result,
        computation_time_ms: elapsed,
    }))
}
```

### 4.4 Configuration File

```toml
# config.toml
[attention]
default_type = "multi-head"
num_heads = 8
hidden_dim = 128
dropout = 0.1

[optimization]
use_simd = true
num_threads = 4
batch_size = 64

[server]
host = "0.0.0.0"
port = 8080
max_batch_size = 256
timeout_ms = 5000
max_connections = 1000

[logging]
level = "info"
format = "json"
```

---

## 5. SDK Design

### 5.1 High-Level Rust SDK

```rust
// src/sdk/mod.rs
use crate::*;

pub mod prelude {
    pub use super::{
        AttentionBuilder,
        AttentionType,
        AttentionConfig,
    };
    pub use crate::{
        ScaledDotProduct,
        MultiHeadAttention,
        HyperbolicAttention,
    };
}

#[derive(Debug, Clone)]
pub enum AttentionType {
    ScaledDotProduct { neighbors: usize },
    MultiHead { num_heads: usize },
    Hyperbolic { curvature: f32 },
    Linear,
    Auto, // Auto-select based on input
}

#[derive(Debug, Clone)]
pub struct AttentionConfig {
    pub hidden_dim: usize,
    pub dropout: f32,
    pub use_simd: bool,
}

impl Default for AttentionConfig {
    fn default() -> Self {
        Self {
            hidden_dim: 128,
            dropout: 0.0,
            use_simd: true,
        }
    }
}

pub struct AttentionBuilder {
    attention_type: Option<AttentionType>,
    config: AttentionConfig,
}

impl AttentionBuilder {
    pub fn new() -> Self {
        Self {
            attention_type: None,
            config: AttentionConfig::default(),
        }
    }

    pub fn attention_type(mut self, att_type: AttentionType) -> Self {
        self.attention_type = Some(att_type);
        self
    }

    pub fn hidden_dim(mut self, dim: usize) -> Self {
        self.config.hidden_dim = dim;
        self
    }

    pub fn dropout(mut self, dropout: f32) -> Self {
        self.config.dropout = dropout;
        self
    }

    pub fn build(self) -> Result<Box<dyn Attention>> {
        let att_type = self.attention_type
            .ok_or_else(|| anyhow::anyhow!("Attention type not specified"))?;

        let attention: Box<dyn Attention> = match att_type {
            AttentionType::ScaledDotProduct { neighbors } => {
                Box::new(ScaledDotProduct::new(self.config.hidden_dim))
            },
            AttentionType::MultiHead { num_heads } => {
                Box::new(MultiHeadAttention::new(
                    num_heads,
                    self.config.hidden_dim,
                    self.config.dropout,
                )?)
            },
            AttentionType::Hyperbolic { curvature } => {
                Box::new(HyperbolicAttention::new(curvature))
            },
            AttentionType::Auto => {
                // Auto-select based on configuration
                Box::new(MultiHeadAttention::new(8, self.config.hidden_dim, 0.0)?)
            },
            _ => unimplemented!(),
        };

        Ok(attention)
    }
}

// Common trait for all attention mechanisms
pub trait Attention {
    fn forward(
        &self,
        query: &[f32],
        keys: &[f32],
        values: &[f32],
    ) -> Result<Vec<f32>>;
}
```

### 5.2 JavaScript/TypeScript SDK

```typescript
// sdk/typescript/src/index.ts
import {
    ScaledDotProductAttention,
    MultiHeadAttention,
    HyperbolicAttention,
} from 'ruvector-attention';

export enum AttentionType {
    ScaledDotProduct = 'scaled-dot-product',
    MultiHead = 'multi-head',
    Hyperbolic = 'hyperbolic',
    Linear = 'linear',
    Auto = 'auto',
}

export interface AttentionConfig {
    type: AttentionType;
    hiddenDim: number;
    dropout?: number;
    numHeads?: number;
    neighbors?: number;
    curvature?: number;
}

export class Attention {
    private inner: any;
    private config: AttentionConfig;

    constructor(config: AttentionConfig) {
        this.config = config;
        this.inner = this.createAttention(config);
    }

    private createAttention(config: AttentionConfig) {
        switch (config.type) {
            case AttentionType.ScaledDotProduct:
                return new ScaledDotProductAttention(config.hiddenDim);

            case AttentionType.MultiHead:
                return MultiHeadAttention.fromParams(
                    config.numHeads || 8,
                    config.hiddenDim,
                    config.dropout || 0.0
                );

            case AttentionType.Hyperbolic:
                return new HyperbolicAttention(config.curvature || 1.0);

            case AttentionType.Auto:
                // Auto-select based on input size
                return MultiHeadAttention.fromParams(8, config.hiddenDim);

            default:
                throw new Error(`Unknown attention type: ${config.type}`);
        }
    }

    forward(
        query: Float32Array,
        keys: Float32Array,
        values: Float32Array,
        options?: { mask?: boolean[], neighbors?: number }
    ): Float32Array {
        if (this.config.type === AttentionType.ScaledDotProduct) {
            return this.inner.forward(
                query,
                keys,
                values,
                options?.neighbors || 100
            );
        } else {
            return this.inner.forward(query, keys, values, options?.mask);
        }
    }

    async forwardAsync(
        query: Float32Array,
        keys: Float32Array,
        values: Float32Array,
        options?: { mask?: boolean[], neighbors?: number }
    ): Promise<Float32Array> {
        if (this.config.type === AttentionType.ScaledDotProduct) {
            return this.inner.forwardAsync(
                query,
                keys,
                values,
                options?.neighbors || 100
            );
        } else {
            return this.inner.forwardAsync(query, keys, values, options?.mask);
        }
    }

    // Streaming API for large inputs
    async *forwardStream(
        queryStream: AsyncIterable<Float32Array>,
        keys: Float32Array,
        values: Float32Array
    ): AsyncGenerator<Float32Array> {
        for await (const query of queryStream) {
            yield await this.forwardAsync(query, keys, values);
        }
    }
}

// Builder pattern
export class AttentionBuilder {
    private config: Partial<AttentionConfig> = {};

    type(type: AttentionType, options?: {
        numHeads?: number,
        neighbors?: number,
        curvature?: number
    }): this {
        this.config.type = type;
        if (options?.numHeads) this.config.numHeads = options.numHeads;
        if (options?.neighbors) this.config.neighbors = options.neighbors;
        if (options?.curvature) this.config.curvature = options.curvature;
        return this;
    }

    hiddenDim(dim: number): this {
        this.config.hiddenDim = dim;
        return this;
    }

    dropout(dropout: number): this {
        this.config.dropout = dropout;
        return this;
    }

    build(): Attention {
        if (!this.config.type) {
            throw new Error('Attention type not specified');
        }
        if (!this.config.hiddenDim) {
            throw new Error('Hidden dimension not specified');
        }

        return new Attention(this.config as AttentionConfig);
    }
}

// Export everything
export {
    ScaledDotProductAttention,
    MultiHeadAttention,
    HyperbolicAttention,
};
```

---

## 6. Testing Strategy

### 6.1 Platform-Specific Tests

```rust
// WASM tests
#[cfg(target_arch = "wasm32")]
mod wasm_tests {
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_scaled_dot_product() {
        // Test WASM-specific behavior
    }
}

// NAPI tests
#[cfg(all(not(target_arch = "wasm32"), feature = "napi"))]
mod napi_tests {
    use napi::bindgen_prelude::*;

    #[test]
    fn test_napi_conversion() {
        // Test NAPI-specific behavior
    }
}
```

### 6.2 Integration Tests

```typescript
// __tests__/integration.test.ts
import { AttentionBuilder, AttentionType } from 'ruvector-attention';

describe('Attention Integration Tests', () => {
    test('scaled dot-product attention', async () => {
        const attention = new AttentionBuilder()
            .type(AttentionType.ScaledDotProduct, { neighbors: 100 })
            .hiddenDim(128)
            .build();

        const query = new Float32Array(128).fill(1.0);
        const keys = new Float32Array(128 * 100).fill(0.5);
        const values = new Float32Array(128 * 100).fill(0.5);

        const result = await attention.forwardAsync(query, keys, values);

        expect(result).toBeInstanceOf(Float32Array);
        expect(result.length).toBe(128);
    });
});
```

---

## 7. Documentation

### 7.1 API Documentation

- Rust: Generated via `cargo doc`
- WASM: Auto-generated TypeScript definitions
- Node.js: TypeScript definitions + JSDoc
- CLI: Auto-generated from Clap

### 7.2 Examples

Each platform should include:
- Basic usage examples
- Advanced patterns
- Performance optimization guides
- Troubleshooting guides

---

## 8. Release Process

### 8.1 Version Management

- Rust crate: `cargo release`
- WASM package: `wasm-pack publish`
- Node.js package: `npm publish`
- CLI binary: GitHub Releases

### 8.2 Distribution

- Rust: crates.io
- WASM: npm (as `ruvector-attention-wasm`)
- Node.js: npm (as `ruvector-attention`)
- CLI: GitHub Releases, Homebrew, Cargo install

---

## 9. Performance Targets

| Platform | Target | Notes |
|----------|--------|-------|
| Rust Native | Baseline | Reference implementation |
| WASM (Browser) | 60-80% of native | JS interop overhead |
| WASM (Server) | 70-90% of native | WASI optimizations |
| Node.js (NAPI) | 95-100% of native | Minimal overhead |
| CLI | 95-100% of native | Direct Rust |

---

## 10. Next Steps

1. Implement WASM bindings (Week 1-2)
2. Implement NAPI-RS bindings (Week 2-3)
3. Build CLI interface (Week 3-4)
4. Create SDK wrappers (Week 4)
5. Write comprehensive tests (Week 5)
6. Documentation and examples (Week 6)
7. Release and distribution (Week 7)

---

## Appendix A: Complete Build Commands

```bash
# Rust native
cargo build --release

# WASM (all targets)
./scripts/build-wasm.sh

# Node.js (all platforms)
cd crates/ruvector-attention-node
pnpm build --platform

# CLI
cargo build --release --bin ruvector-attention

# All at once
./scripts/build-all.sh
```

## Appendix B: Platform-Specific Optimizations

### WASM Optimizations
- Enable `wasm-opt` for size reduction
- Use SIMD128 where supported
- Minimize JS/WASM boundary crossings

### NAPI Optimizations
- Use `AsyncTask` for CPU-intensive operations
- Minimize allocations in hot paths
- Leverage native Node.js buffers

### CLI Optimizations
- Use `mimalloc` for better allocation performance
- Enable LTO and aggressive optimizations
- Consider static linking for distribution

---

**Document Status:** Implementation Ready
**Last Updated:** 2025-11-30
**Review Date:** 2025-12-07
