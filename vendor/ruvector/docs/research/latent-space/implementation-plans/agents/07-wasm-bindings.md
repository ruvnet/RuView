# Agent 07: WASM Bindings Implementation Plan

**Agent ID:** 07-wasm-bindings
**Version:** 1.0.0
**Date:** 2025-11-30
**Status:** Implementation Ready
**Dependencies:** Agent 01 (Core Attention), Agent 02 (GNN Integration)

---

## Executive Summary

This document provides a complete implementation plan for WebAssembly bindings for RuVector's attention mechanisms. The agent will create production-ready WASM bindings using wasm-bindgen, enabling browser and Node.js deployment with TypeScript support, optimal memory management, and zero-copy data transfers where possible.

---

## 1. Project Setup

### 1.1 Directory Structure

```
crates/ruvector-attention-wasm/
├── Cargo.toml
├── README.md
├── package.json
├── tsconfig.json
├── src/
│   ├── lib.rs                    # Main WASM exports
│   ├── attention/
│   │   ├── mod.rs
│   │   ├── scaled_dot.rs         # ScaledDotProduct bindings
│   │   ├── multi_head.rs         # MultiHead bindings
│   │   ├── hyperbolic.rs         # Hyperbolic bindings
│   │   ├── linear.rs             # Linear bindings
│   │   ├── cross.rs              # Cross attention bindings
│   │   ├── self_attention.rs     # Self attention bindings
│   │   └── flash.rs              # Flash attention bindings
│   ├── gnn/
│   │   ├── mod.rs
│   │   ├── graph.rs              # Graph WASM bindings
│   │   └── messaging.rs          # Message passing bindings
│   ├── utils.rs                  # Memory, logging utilities
│   ├── error.rs                  # Error handling
│   ├── types.rs                  # JS-compatible types
│   └── async_ops.rs              # Promise-based async
├── js/
│   ├── index.ts                  # TypeScript wrapper
│   ├── types.ts                  # Type definitions
│   └── utils.ts                  # JS utilities
├── tests/
│   ├── wasm.rs                   # Rust-side tests
│   └── integration.test.ts       # JS integration tests
├── examples/
│   ├── browser/
│   │   ├── index.html
│   │   ├── demo.ts
│   │   └── worker.ts
│   └── node/
│       ├── basic.mjs
│       └── performance.mjs
└── scripts/
    ├── build.sh
    ├── test.sh
    └── release.sh
```

### 1.2 Cargo.toml Configuration

```toml
# crates/ruvector-attention-wasm/Cargo.toml
[package]
name = "ruvector-attention-wasm"
version = "0.1.0"
authors = ["RuVector Team"]
edition = "2021"
license = "MIT OR Apache-2.0"
description = "WebAssembly bindings for RuVector attention mechanisms"
repository = "https://github.com/ruvnet/ruvector"
keywords = ["wasm", "attention", "machine-learning", "gnn", "webassembly"]
categories = ["wasm", "science"]

[lib]
crate-type = ["cdylib", "rlib"]

[features]
default = ["console_error_panic_hook"]
# Performance features
simd = ["ruvector-attention/simd"]
parallel = ["wasm-bindgen-rayon"]
# Optimization features
size-optimized = []
speed-optimized = []
# Debug features
debug-logging = ["web-sys/console"]

[dependencies]
# Core dependencies
ruvector-attention = { path = "../ruvector-attention" }
ruvector-gnn = { path = "../ruvector-gnn", optional = true }

# WASM bindings
wasm-bindgen = "0.2.92"
wasm-bindgen-futures = "0.4.42"
js-sys = "0.3.69"

# Web APIs
web-sys = { version = "0.3.69", features = [
    "console",
    "Performance",
    "PerformanceTiming",
    "PerformanceMeasure",
    "Window",
    "Worker",
    "MessageEvent",
    "WorkerGlobalScope",
] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde-wasm-bindgen = "0.6.5"
serde_json = "1.0"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Utilities
console_error_panic_hook = { version = "0.1.7", optional = true }
wee_alloc = { version = "0.4.5", optional = true }
wasm-bindgen-rayon = { version = "1.2", optional = true }

# Logging
log = "0.4"
console_log = { version = "1.0", features = ["color"] }

[dev-dependencies]
wasm-bindgen-test = "0.3.42"
criterion = "0.5"
approx = "0.5"

[profile.release]
opt-level = "z"           # Optimize for size
lto = true                # Link Time Optimization
codegen-units = 1         # Better optimization
panic = "abort"           # Smaller binary
strip = true              # Strip symbols

[profile.release-speed]
inherits = "release"
opt-level = 3             # Optimize for speed
```

### 1.3 Package.json Configuration

```json
{
  "name": "@ruvector/attention-wasm",
  "version": "0.1.0",
  "description": "WebAssembly bindings for RuVector attention mechanisms",
  "main": "dist/index.js",
  "module": "dist/index.mjs",
  "types": "dist/index.d.ts",
  "files": [
    "dist/**/*",
    "README.md",
    "LICENSE"
  ],
  "scripts": {
    "build": "./scripts/build.sh",
    "build:dev": "wasm-pack build --dev --target web",
    "build:release": "wasm-pack build --release --target bundler",
    "build:node": "wasm-pack build --release --target nodejs",
    "build:all": "./scripts/build.sh --all-targets",
    "test": "npm run test:wasm && npm run test:js",
    "test:wasm": "wasm-pack test --headless --firefox --chrome",
    "test:js": "jest",
    "bench": "wasm-pack test --release --headless --firefox -- --bench",
    "docs": "typedoc --out docs js/index.ts",
    "prepublishOnly": "npm run build:all && npm test"
  },
  "repository": {
    "type": "git",
    "url": "https://github.com/ruvnet/ruvector.git"
  },
  "keywords": [
    "wasm",
    "webassembly",
    "attention",
    "machine-learning",
    "gnn",
    "graph-neural-networks"
  ],
  "author": "RuVector Team",
  "license": "MIT OR Apache-2.0",
  "devDependencies": {
    "@types/jest": "^29.5.0",
    "@types/node": "^20.0.0",
    "jest": "^29.5.0",
    "ts-jest": "^29.1.0",
    "typedoc": "^0.25.0",
    "typescript": "^5.3.0",
    "wasm-pack": "^0.12.0"
  },
  "dependencies": {
    "tslib": "^2.6.0"
  }
}
```

### 1.4 TypeScript Configuration

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true,
    "outDir": "./dist",
    "rootDir": "./js",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "moduleResolution": "node",
    "resolveJsonModule": true,
    "allowSyntheticDefaultImports": true
  },
  "include": ["js/**/*"],
  "exclude": ["node_modules", "dist", "pkg"]
}
```

---

## 2. Core WASM Bindings Implementation

### 2.1 Main Library Entry (lib.rs)

```rust
// crates/ruvector-attention-wasm/src/lib.rs
use wasm_bindgen::prelude::*;

// Re-export modules
pub mod attention;
pub mod error;
pub mod types;
pub mod utils;
pub mod async_ops;

#[cfg(feature = "gnn")]
pub mod gnn;

// Initialize panic hook and logging
#[wasm_bindgen(start)]
pub fn init() {
    // Set panic hook for better error messages
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    // Initialize logging
    console_log::init_with_level(log::Level::Info)
        .expect("Failed to initialize logger");

    log::info!("RuVector WASM initialized");
}

// Export version information
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// Memory utilities
#[wasm_bindgen]
pub fn memory_usage() -> JsValue {
    let usage = utils::get_memory_usage();
    serde_wasm_bindgen::to_value(&usage).unwrap()
}
```

### 2.2 Error Handling (error.rs)

```rust
// crates/ruvector-attention-wasm/src/error.rs
use wasm_bindgen::prelude::*;
use thiserror::Error;

/// WASM-compatible error type
#[derive(Error, Debug)]
pub enum WasmError {
    #[error("Invalid dimensions: {0}")]
    InvalidDimensions(String),

    #[error("Invalid attention configuration: {0}")]
    InvalidConfig(String),

    #[error("Computation failed: {0}")]
    ComputationError(String),

    #[error("Memory error: {0}")]
    MemoryError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

impl From<WasmError> for JsValue {
    fn from(error: WasmError) -> Self {
        JsValue::from_str(&error.to_string())
    }
}

/// Result type for WASM operations
pub type WasmResult<T> = Result<T, WasmError>;

/// Convert Rust errors to JS errors
#[wasm_bindgen]
pub struct WasmErrorInfo {
    message: String,
    error_type: String,
}

#[wasm_bindgen]
impl WasmErrorInfo {
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn error_type(&self) -> String {
        self.error_type.clone()
    }
}
```

### 2.3 Type Definitions (types.rs)

```rust
// crates/ruvector-attention-wasm/src/types.rs
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

/// Configuration for attention mechanisms
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionConfig {
    pub(crate) num_heads: usize,
    pub(crate) head_dim: usize,
    pub(crate) dropout: f32,
    pub(crate) use_bias: bool,
}

#[wasm_bindgen]
impl AttentionConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(num_heads: usize, head_dim: usize) -> Self {
        Self {
            num_heads,
            head_dim,
            dropout: 0.0,
            use_bias: true,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn num_heads(&self) -> usize {
        self.num_heads
    }

    #[wasm_bindgen(getter)]
    pub fn head_dim(&self) -> usize {
        self.head_dim
    }

    #[wasm_bindgen(setter)]
    pub fn set_dropout(&mut self, dropout: f32) {
        self.dropout = dropout;
    }

    #[wasm_bindgen(setter)]
    pub fn set_use_bias(&mut self, use_bias: bool) {
        self.use_bias = use_bias;
    }
}

/// Hyperbolic attention configuration
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperbolicConfig {
    pub(crate) curvature: f32,
    pub(crate) manifold_type: String,
    pub(crate) num_heads: usize,
}

#[wasm_bindgen]
impl HyperbolicConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(curvature: f32, num_heads: usize) -> Self {
        Self {
            curvature,
            manifold_type: "poincare".to_string(),
            num_heads,
        }
    }

    #[wasm_bindgen(setter)]
    pub fn set_manifold_type(&mut self, manifold_type: String) {
        self.manifold_type = manifold_type;
    }
}

/// Attention mask type
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MaskType {
    Causal = "causal",
    Padding = "padding",
    None = "none",
}
```

### 2.4 Utilities (utils.rs)

```rust
// crates/ruvector-attention-wasm/src/utils.rs
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use js_sys::{Float32Array, Uint32Array};

/// Memory usage statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryUsage {
    pub allocated: usize,
    pub total: usize,
}

#[wasm_bindgen]
pub fn get_memory_usage() -> JsValue {
    // Get WASM memory statistics
    let allocated = 0; // Placeholder - implement actual tracking
    let total = wasm_bindgen::memory().buffer().byte_length() as usize;

    let usage = MemoryUsage { allocated, total };
    serde_wasm_bindgen::to_value(&usage).unwrap()
}

/// Convert Float32Array to Vec<f32>
pub fn f32_array_to_vec(array: &Float32Array) -> Vec<f32> {
    let mut vec = vec![0.0; array.length() as usize];
    array.copy_to(&mut vec);
    vec
}

/// Convert Vec<f32> to Float32Array (zero-copy when possible)
pub fn vec_to_f32_array(vec: &[f32]) -> Float32Array {
    unsafe { Float32Array::view(vec) }
}

/// Validate tensor dimensions
pub fn validate_dimensions(
    query_shape: &[usize],
    key_shape: &[usize],
    value_shape: &[usize],
) -> Result<(), String> {
    if query_shape.len() != 3 || key_shape.len() != 3 || value_shape.len() != 3 {
        return Err("All tensors must be 3-dimensional".to_string());
    }

    if query_shape[0] != key_shape[0] || query_shape[0] != value_shape[0] {
        return Err("Batch sizes must match".to_string());
    }

    if query_shape[2] != key_shape[2] {
        return Err("Query and key dimensions must match".to_string());
    }

    if key_shape[1] != value_shape[1] {
        return Err("Key and value sequence lengths must match".to_string());
    }

    Ok(())
}

/// Performance timer
#[wasm_bindgen]
pub struct PerfTimer {
    start: f64,
}

#[wasm_bindgen]
impl PerfTimer {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let start = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        Self { start }
    }

    pub fn elapsed(&self) -> f64 {
        web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now() - self.start)
            .unwrap_or(0.0)
    }
}
```

---

## 3. Attention Mechanism Bindings

### 3.1 Scaled Dot-Product Attention (attention/scaled_dot.rs)

```rust
// crates/ruvector-attention-wasm/src/attention/scaled_dot.rs
use wasm_bindgen::prelude::*;
use js_sys::Float32Array;
use ruvector_attention::ScaledDotProductAttention;
use crate::error::{WasmError, WasmResult};
use crate::utils::{f32_array_to_vec, vec_to_f32_array, validate_dimensions};

#[wasm_bindgen]
pub struct WasmScaledDotProduct {
    inner: ScaledDotProductAttention,
    head_dim: usize,
}

#[wasm_bindgen]
impl WasmScaledDotProduct {
    /// Create a new scaled dot-product attention mechanism
    #[wasm_bindgen(constructor)]
    pub fn new(head_dim: usize, dropout: Option<f32>) -> Self {
        let inner = ScaledDotProductAttention::new(head_dim, dropout.unwrap_or(0.0));
        Self { inner, head_dim }
    }

    /// Forward pass with Float32Arrays
    ///
    /// # Arguments
    /// * `query` - Query tensor as Float32Array (batch_size * seq_len * head_dim)
    /// * `key` - Key tensor as Float32Array (batch_size * seq_len * head_dim)
    /// * `value` - Value tensor as Float32Array (batch_size * seq_len * head_dim)
    /// * `query_shape` - Shape of query [batch, seq_len, dim]
    /// * `key_shape` - Shape of key [batch, seq_len, dim]
    /// * `value_shape` - Shape of value [batch, seq_len, dim]
    /// * `mask` - Optional attention mask
    ///
    /// # Returns
    /// Float32Array containing the attention output
    #[wasm_bindgen]
    pub fn forward(
        &self,
        query: &Float32Array,
        key: &Float32Array,
        value: &Float32Array,
        query_shape: Vec<usize>,
        key_shape: Vec<usize>,
        value_shape: Vec<usize>,
        mask: Option<Float32Array>,
    ) -> Result<Float32Array, JsValue> {
        // Validate dimensions
        validate_dimensions(&query_shape, &key_shape, &value_shape)
            .map_err(|e| WasmError::InvalidDimensions(e))?;

        // Convert to Vec
        let q = f32_array_to_vec(query);
        let k = f32_array_to_vec(key);
        let v = f32_array_to_vec(value);
        let m = mask.map(|m| f32_array_to_vec(&m));

        // Reshape to 3D tensors (batch, seq, dim)
        let batch_size = query_shape[0];
        let seq_len_q = query_shape[1];
        let seq_len_k = key_shape[1];

        // Perform attention computation
        let output = self.inner
            .forward(&q, &k, &v, m.as_deref())
            .map_err(|e| WasmError::ComputationError(e.to_string()))?;

        // Convert back to Float32Array
        Ok(vec_to_f32_array(&output))
    }

    /// Compute attention weights only (no value projection)
    #[wasm_bindgen]
    pub fn attention_weights(
        &self,
        query: &Float32Array,
        key: &Float32Array,
        query_shape: Vec<usize>,
        key_shape: Vec<usize>,
        mask: Option<Float32Array>,
    ) -> Result<Float32Array, JsValue> {
        let q = f32_array_to_vec(query);
        let k = f32_array_to_vec(key);
        let m = mask.map(|m| f32_array_to_vec(&m));

        let weights = self.inner
            .compute_attention_weights(&q, &k, m.as_deref())
            .map_err(|e| WasmError::ComputationError(e.to_string()))?;

        Ok(vec_to_f32_array(&weights))
    }

    /// Get the scaling factor used
    #[wasm_bindgen(getter)]
    pub fn scale(&self) -> f32 {
        self.inner.scale()
    }
}
```

### 3.2 Multi-Head Attention (attention/multi_head.rs)

```rust
// crates/ruvector-attention-wasm/src/attention/multi_head.rs
use wasm_bindgen::prelude::*;
use js_sys::Float32Array;
use ruvector_attention::MultiHeadAttention;
use crate::error::{WasmError, WasmResult};
use crate::types::AttentionConfig;
use crate::utils::{f32_array_to_vec, vec_to_f32_array};

#[wasm_bindgen]
pub struct WasmMultiHeadAttention {
    inner: MultiHeadAttention,
    config: AttentionConfig,
}

#[wasm_bindgen]
impl WasmMultiHeadAttention {
    /// Create a new multi-head attention mechanism
    #[wasm_bindgen(constructor)]
    pub fn new(config: AttentionConfig) -> Result<WasmMultiHeadAttention, JsValue> {
        let inner = MultiHeadAttention::new(
            config.num_heads,
            config.head_dim,
            config.dropout,
            config.use_bias,
        ).map_err(|e| WasmError::InvalidConfig(e.to_string()))?;

        Ok(Self { inner, config })
    }

    /// Forward pass for multi-head attention
    ///
    /// # Arguments
    /// * `query` - Query tensor (batch * seq_q * model_dim)
    /// * `key` - Key tensor (batch * seq_k * model_dim)
    /// * `value` - Value tensor (batch * seq_v * model_dim)
    /// * `shape` - [batch_size, seq_len_q, seq_len_k, model_dim]
    /// * `mask` - Optional attention mask
    ///
    /// # Returns
    /// Tuple of (output, attention_weights)
    #[wasm_bindgen]
    pub fn forward(
        &mut self,
        query: &Float32Array,
        key: &Float32Array,
        value: &Float32Array,
        shape: Vec<usize>,
        mask: Option<Float32Array>,
    ) -> Result<JsValue, JsValue> {
        if shape.len() != 4 {
            return Err(WasmError::InvalidDimensions(
                "Shape must be [batch, seq_q, seq_k, model_dim]".to_string()
            ).into());
        }

        let q = f32_array_to_vec(query);
        let k = f32_array_to_vec(key);
        let v = f32_array_to_vec(value);
        let m = mask.map(|m| f32_array_to_vec(&m));

        let (output, weights) = self.inner
            .forward(&q, &k, &v, m.as_deref())
            .map_err(|e| WasmError::ComputationError(e.to_string()))?;

        // Create result object
        let result = js_sys::Object::new();
        js_sys::Reflect::set(
            &result,
            &JsValue::from_str("output"),
            &vec_to_f32_array(&output),
        )?;
        js_sys::Reflect::set(
            &result,
            &JsValue::from_str("weights"),
            &vec_to_f32_array(&weights),
        )?;

        Ok(result.into())
    }

    /// Get configuration
    #[wasm_bindgen(getter)]
    pub fn config(&self) -> AttentionConfig {
        self.config.clone()
    }

    /// Get number of parameters
    #[wasm_bindgen]
    pub fn num_parameters(&self) -> usize {
        self.inner.num_parameters()
    }
}
```

### 3.3 Hyperbolic Attention (attention/hyperbolic.rs)

```rust
// crates/ruvector-attention-wasm/src/attention/hyperbolic.rs
use wasm_bindgen::prelude::*;
use js_sys::Float32Array;
use ruvector_attention::HyperbolicAttention;
use crate::error::{WasmError, WasmResult};
use crate::types::HyperbolicConfig;
use crate::utils::{f32_array_to_vec, vec_to_f32_array};

#[wasm_bindgen]
pub struct WasmHyperbolicAttention {
    inner: HyperbolicAttention,
    config: HyperbolicConfig,
}

#[wasm_bindgen]
impl WasmHyperbolicAttention {
    /// Create hyperbolic attention mechanism
    #[wasm_bindgen(constructor)]
    pub fn new(config: HyperbolicConfig) -> Result<WasmHyperbolicAttention, JsValue> {
        let inner = HyperbolicAttention::new(
            config.curvature,
            config.num_heads,
            &config.manifold_type,
        ).map_err(|e| WasmError::InvalidConfig(e.to_string()))?;

        Ok(Self { inner, config })
    }

    /// Forward pass in hyperbolic space
    #[wasm_bindgen]
    pub fn forward(
        &self,
        query: &Float32Array,
        key: &Float32Array,
        value: &Float32Array,
        shape: Vec<usize>,
    ) -> Result<Float32Array, JsValue> {
        let q = f32_array_to_vec(query);
        let k = f32_array_to_vec(key);
        let v = f32_array_to_vec(value);

        let output = self.inner
            .forward(&q, &k, &v)
            .map_err(|e| WasmError::ComputationError(e.to_string()))?;

        Ok(vec_to_f32_array(&output))
    }

    /// Compute hyperbolic distance matrix
    #[wasm_bindgen]
    pub fn distance_matrix(
        &self,
        points_a: &Float32Array,
        points_b: &Float32Array,
    ) -> Result<Float32Array, JsValue> {
        let a = f32_array_to_vec(points_a);
        let b = f32_array_to_vec(points_b);

        let distances = self.inner
            .compute_distances(&a, &b)
            .map_err(|e| WasmError::ComputationError(e.to_string()))?;

        Ok(vec_to_f32_array(&distances))
    }

    /// Project to Poincaré ball
    #[wasm_bindgen]
    pub fn project_to_ball(
        &self,
        points: &Float32Array,
    ) -> Result<Float32Array, JsValue> {
        let pts = f32_array_to_vec(points);
        let projected = self.inner
            .project(&pts)
            .map_err(|e| WasmError::ComputationError(e.to_string()))?;

        Ok(vec_to_f32_array(&projected))
    }

    #[wasm_bindgen(getter)]
    pub fn curvature(&self) -> f32 {
        self.config.curvature
    }
}
```

### 3.4 Linear Attention (attention/linear.rs)

```rust
// crates/ruvector-attention-wasm/src/attention/linear.rs
use wasm_bindgen::prelude::*;
use js_sys::Float32Array;
use ruvector_attention::LinearAttention;
use crate::error::WasmError;
use crate::utils::{f32_array_to_vec, vec_to_f32_array};

#[wasm_bindgen]
pub struct WasmLinearAttention {
    inner: LinearAttention,
    feature_dim: usize,
}

#[wasm_bindgen]
impl WasmLinearAttention {
    /// Create linear attention with O(n) complexity
    #[wasm_bindgen(constructor)]
    pub fn new(model_dim: usize, feature_dim: usize) -> Result<WasmLinearAttention, JsValue> {
        let inner = LinearAttention::new(model_dim, feature_dim)
            .map_err(|e| WasmError::InvalidConfig(e.to_string()))?;

        Ok(Self { inner, feature_dim })
    }

    /// Forward pass with linear complexity
    #[wasm_bindgen]
    pub fn forward(
        &self,
        query: &Float32Array,
        key: &Float32Array,
        value: &Float32Array,
        shape: Vec<usize>, // [batch, seq, dim]
    ) -> Result<Float32Array, JsValue> {
        let q = f32_array_to_vec(query);
        let k = f32_array_to_vec(key);
        let v = f32_array_to_vec(value);

        let output = self.inner
            .forward(&q, &k, &v)
            .map_err(|e| WasmError::ComputationError(e.to_string()))?;

        Ok(vec_to_f32_array(&output))
    }

    /// Apply feature map (kernel approximation)
    #[wasm_bindgen]
    pub fn feature_map(
        &self,
        input: &Float32Array,
    ) -> Result<Float32Array, JsValue> {
        let inp = f32_array_to_vec(input);
        let features = self.inner
            .apply_feature_map(&inp)
            .map_err(|e| WasmError::ComputationError(e.to_string()))?;

        Ok(vec_to_f32_array(&features))
    }

    #[wasm_bindgen(getter)]
    pub fn feature_dim(&self) -> usize {
        self.feature_dim
    }
}
```

### 3.5 Cross Attention (attention/cross.rs)

```rust
// crates/ruvector-attention-wasm/src/attention/cross.rs
use wasm_bindgen::prelude::*;
use js_sys::Float32Array;
use ruvector_attention::CrossAttention;
use crate::error::WasmError;
use crate::types::AttentionConfig;
use crate::utils::{f32_array_to_vec, vec_to_f32_array};

#[wasm_bindgen]
pub struct WasmCrossAttention {
    inner: CrossAttention,
    config: AttentionConfig,
}

#[wasm_bindgen]
impl WasmCrossAttention {
    #[wasm_bindgen(constructor)]
    pub fn new(config: AttentionConfig) -> Result<WasmCrossAttention, JsValue> {
        let inner = CrossAttention::new(
            config.num_heads,
            config.head_dim,
            config.dropout,
        ).map_err(|e| WasmError::InvalidConfig(e.to_string()))?;

        Ok(Self { inner, config })
    }

    /// Cross attention between two sequences
    /// Typically used in encoder-decoder architectures
    #[wasm_bindgen]
    pub fn forward(
        &mut self,
        query: &Float32Array,      // From decoder
        key: &Float32Array,        // From encoder
        value: &Float32Array,      // From encoder
        query_shape: Vec<usize>,   // [batch, dec_len, dim]
        kv_shape: Vec<usize>,      // [batch, enc_len, dim]
        mask: Option<Float32Array>,
    ) -> Result<Float32Array, JsValue> {
        let q = f32_array_to_vec(query);
        let k = f32_array_to_vec(key);
        let v = f32_array_to_vec(value);
        let m = mask.map(|m| f32_array_to_vec(&m));

        let output = self.inner
            .forward(&q, &k, &v, m.as_deref())
            .map_err(|e| WasmError::ComputationError(e.to_string()))?;

        Ok(vec_to_f32_array(&output))
    }
}
```

### 3.6 Flash Attention (attention/flash.rs)

```rust
// crates/ruvector-attention-wasm/src/attention/flash.rs
use wasm_bindgen::prelude::*;
use js_sys::Float32Array;
use ruvector_attention::FlashAttention;
use crate::error::WasmError;
use crate::utils::{f32_array_to_vec, vec_to_f32_array};

#[wasm_bindgen]
pub struct WasmFlashAttention {
    inner: FlashAttention,
    block_size: usize,
}

#[wasm_bindgen]
impl WasmFlashAttention {
    /// Create Flash Attention with memory-efficient tiling
    #[wasm_bindgen(constructor)]
    pub fn new(head_dim: usize, block_size: Option<usize>) -> Result<WasmFlashAttention, JsValue> {
        let block_size = block_size.unwrap_or(64);
        let inner = FlashAttention::new(head_dim, block_size)
            .map_err(|e| WasmError::InvalidConfig(e.to_string()))?;

        Ok(Self { inner, block_size })
    }

    /// Memory-efficient attention computation
    #[wasm_bindgen]
    pub fn forward(
        &self,
        query: &Float32Array,
        key: &Float32Array,
        value: &Float32Array,
        shape: Vec<usize>,
    ) -> Result<Float32Array, JsValue> {
        let q = f32_array_to_vec(query);
        let k = f32_array_to_vec(key);
        let v = f32_array_to_vec(value);

        let output = self.inner
            .forward(&q, &k, &v)
            .map_err(|e| WasmError::ComputationError(e.to_string()))?;

        Ok(vec_to_f32_array(&output))
    }

    #[wasm_bindgen(getter)]
    pub fn block_size(&self) -> usize {
        self.block_size
    }
}
```

### 3.7 Module Export (attention/mod.rs)

```rust
// crates/ruvector-attention-wasm/src/attention/mod.rs
pub mod scaled_dot;
pub mod multi_head;
pub mod hyperbolic;
pub mod linear;
pub mod cross;
pub mod flash;

pub use scaled_dot::WasmScaledDotProduct;
pub use multi_head::WasmMultiHeadAttention;
pub use hyperbolic::WasmHyperbolicAttention;
pub use linear::WasmLinearAttention;
pub use cross::WasmCrossAttention;
pub use flash::WasmFlashAttention;
```

---

## 4. Async Operations

### 4.1 Promise-based API (async_ops.rs)

```rust
// crates/ruvector-attention-wasm/src/async_ops.rs
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
use js_sys::{Float32Array, Promise};
use crate::attention::*;
use crate::error::WasmError;
use crate::utils::{f32_array_to_vec, vec_to_f32_array};

/// Async wrapper for multi-head attention
#[wasm_bindgen]
pub struct AsyncMultiHeadAttention {
    inner: WasmMultiHeadAttention,
}

#[wasm_bindgen]
impl AsyncMultiHeadAttention {
    #[wasm_bindgen(constructor)]
    pub fn new(config: crate::types::AttentionConfig) -> Result<AsyncMultiHeadAttention, JsValue> {
        Ok(Self {
            inner: WasmMultiHeadAttention::new(config)?,
        })
    }

    /// Async forward pass - returns Promise
    #[wasm_bindgen]
    pub fn forward_async(
        &mut self,
        query: Float32Array,
        key: Float32Array,
        value: Float32Array,
        shape: Vec<usize>,
        mask: Option<Float32Array>,
    ) -> Promise {
        let mut inner = self.inner.clone();

        future_to_promise(async move {
            // Perform computation in a "future" context
            // In WASM this doesn't create real threads but yields to event loop
            let result = inner.forward(&query, &key, &value, shape, mask)?;
            Ok(result)
        })
    }
}

/// Batch processing for multiple attention operations
#[wasm_bindgen]
pub struct BatchProcessor {
    configs: Vec<crate::types::AttentionConfig>,
}

#[wasm_bindgen]
impl BatchProcessor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            configs: Vec::new(),
        }
    }

    /// Process multiple attention operations in sequence
    /// Returns Promise that resolves to array of results
    #[wasm_bindgen]
    pub fn process_batch(
        &self,
        inputs: JsValue, // Array of input objects
    ) -> Promise {
        future_to_promise(async move {
            // Parse inputs and process each
            let results = js_sys::Array::new();
            // Implementation here
            Ok(results.into())
        })
    }
}
```

---

## 5. TypeScript Wrapper and Type Definitions

### 5.1 Main TypeScript Entry (js/index.ts)

```typescript
// js/index.ts
import init, * as wasm from '../pkg/ruvector_attention_wasm';

// Re-export WASM types
export {
  WasmScaledDotProduct,
  WasmMultiHeadAttention,
  WasmHyperbolicAttention,
  WasmLinearAttention,
  WasmCrossAttention,
  WasmFlashAttention,
  AttentionConfig,
  HyperbolicConfig,
  MaskType,
  PerfTimer,
} from '../pkg/ruvector_attention_wasm';

// Type definitions
export interface Tensor {
  data: Float32Array;
  shape: number[];
}

export interface AttentionOutput {
  output: Float32Array;
  weights?: Float32Array;
}

export interface MemoryUsage {
  allocated: number;
  total: number;
}

// High-level API wrapper
export class RuVectorAttention {
  private initialized: boolean = false;

  /**
   * Initialize the WASM module
   * Must be called before using any attention mechanisms
   */
  async init(): Promise<void> {
    if (!this.initialized) {
      await init();
      this.initialized = true;
      console.log('RuVector WASM initialized, version:', wasm.version());
    }
  }

  /**
   * Create a scaled dot-product attention mechanism
   */
  createScaledDotProduct(headDim: number, dropout?: number): wasm.WasmScaledDotProduct {
    this.checkInitialized();
    return new wasm.WasmScaledDotProduct(headDim, dropout);
  }

  /**
   * Create a multi-head attention mechanism
   */
  createMultiHead(config: wasm.AttentionConfig): wasm.WasmMultiHeadAttention {
    this.checkInitialized();
    return new wasm.WasmMultiHeadAttention(config);
  }

  /**
   * Create hyperbolic attention
   */
  createHyperbolic(config: wasm.HyperbolicConfig): wasm.WasmHyperbolicAttention {
    this.checkInitialized();
    return new wasm.WasmHyperbolicAttention(config);
  }

  /**
   * Create linear attention (O(n) complexity)
   */
  createLinear(modelDim: number, featureDim: number): wasm.WasmLinearAttention {
    this.checkInitialized();
    return new wasm.WasmLinearAttention(modelDim, featureDim);
  }

  /**
   * Create cross attention
   */
  createCross(config: wasm.AttentionConfig): wasm.WasmCrossAttention {
    this.checkInitialized();
    return new wasm.WasmCrossAttention(config);
  }

  /**
   * Create Flash Attention
   */
  createFlash(headDim: number, blockSize?: number): wasm.WasmFlashAttention {
    this.checkInitialized();
    return new wasm.WasmFlashAttention(headDim, blockSize);
  }

  /**
   * Get current memory usage
   */
  getMemoryUsage(): MemoryUsage {
    this.checkInitialized();
    return wasm.memory_usage() as unknown as MemoryUsage;
  }

  private checkInitialized(): void {
    if (!this.initialized) {
      throw new Error('RuVector WASM not initialized. Call init() first.');
    }
  }
}

// Singleton instance
export const ruVector = new RuVectorAttention();

// Helper functions
export function createTensor(data: number[] | Float32Array, shape: number[]): Tensor {
  const flatData = data instanceof Float32Array ? data : new Float32Array(data);
  return { data: flatData, shape };
}

export function validateTensorShape(tensor: Tensor, expectedDims: number): boolean {
  return tensor.shape.length === expectedDims;
}

// Default export
export default ruVector;
```

### 5.2 Type Definitions (js/types.ts)

```typescript
// js/types.ts

/**
 * Configuration for attention mechanisms
 */
export interface IAttentionConfig {
  numHeads: number;
  headDim: number;
  dropout?: number;
  useBias?: boolean;
}

/**
 * Hyperbolic attention configuration
 */
export interface IHyperbolicConfig {
  curvature: number;
  numHeads: number;
  manifoldType?: 'poincare' | 'lorentz' | 'hyperboloid';
}

/**
 * Attention mask types
 */
export enum AttentionMaskType {
  Causal = 'causal',
  Padding = 'padding',
  None = 'none',
}

/**
 * Tensor type for multi-dimensional arrays
 */
export interface ITensor {
  data: Float32Array;
  shape: number[];
  dtype?: 'float32' | 'float64';
}

/**
 * Attention computation result
 */
export interface IAttentionResult {
  output: Float32Array;
  weights?: Float32Array;
  metadata?: {
    computeTime: number;
    memoryUsed: number;
  };
}

/**
 * Performance metrics
 */
export interface IPerformanceMetrics {
  forwardTime: number;
  backwardTime?: number;
  memoryPeak: number;
  flops?: number;
}

/**
 * Batch computation options
 */
export interface IBatchOptions {
  batchSize: number;
  async?: boolean;
  progressCallback?: (progress: number) => void;
}
```

### 5.3 Utility Functions (js/utils.ts)

```typescript
// js/utils.ts

/**
 * Reshape a flat Float32Array into a tensor with given shape
 */
export function reshape(data: Float32Array, shape: number[]): Float32Array {
  const totalSize = shape.reduce((a, b) => a * b, 1);
  if (data.length !== totalSize) {
    throw new Error(`Cannot reshape array of length ${data.length} into shape [${shape}]`);
  }
  return data;
}

/**
 * Create an attention mask for causal (autoregressive) attention
 */
export function createCausalMask(seqLen: number): Float32Array {
  const mask = new Float32Array(seqLen * seqLen);
  for (let i = 0; i < seqLen; i++) {
    for (let j = 0; j < seqLen; j++) {
      mask[i * seqLen + j] = j <= i ? 0.0 : -Infinity;
    }
  }
  return mask;
}

/**
 * Create a padding mask
 */
export function createPaddingMask(lengths: number[], maxLen: number): Float32Array {
  const batchSize = lengths.length;
  const mask = new Float32Array(batchSize * maxLen).fill(-Infinity);

  for (let i = 0; i < batchSize; i++) {
    const len = lengths[i];
    for (let j = 0; j < len; j++) {
      mask[i * maxLen + j] = 0.0;
    }
  }

  return mask;
}

/**
 * Compute tensor statistics (mean, std, min, max)
 */
export interface TensorStats {
  mean: number;
  std: number;
  min: number;
  max: number;
}

export function computeStats(data: Float32Array): TensorStats {
  let sum = 0;
  let min = Infinity;
  let max = -Infinity;

  for (let i = 0; i < data.length; i++) {
    const val = data[i];
    sum += val;
    min = Math.min(min, val);
    max = Math.max(max, val);
  }

  const mean = sum / data.length;

  let sqDiffSum = 0;
  for (let i = 0; i < data.length; i++) {
    const diff = data[i] - mean;
    sqDiffSum += diff * diff;
  }

  const std = Math.sqrt(sqDiffSum / data.length);

  return { mean, std, min, max };
}

/**
 * Normalize tensor to zero mean and unit variance
 */
export function normalize(data: Float32Array): Float32Array {
  const stats = computeStats(data);
  const normalized = new Float32Array(data.length);

  for (let i = 0; i < data.length; i++) {
    normalized[i] = (data[i] - stats.mean) / (stats.std + 1e-8);
  }

  return normalized;
}

/**
 * Performance measurement utility
 */
export class PerformanceMonitor {
  private marks: Map<string, number> = new Map();

  mark(label: string): void {
    this.marks.set(label, performance.now());
  }

  measure(startLabel: string, endLabel?: string): number {
    const start = this.marks.get(startLabel);
    if (!start) {
      throw new Error(`No mark found for: ${startLabel}`);
    }

    const end = endLabel ? this.marks.get(endLabel) : performance.now();
    if (!end) {
      throw new Error(`No mark found for: ${endLabel}`);
    }

    return end - start;
  }

  clear(): void {
    this.marks.clear();
  }
}
```

---

## 6. Build Scripts and Configuration

### 6.1 Build Script (scripts/build.sh)

```bash
#!/bin/bash
# scripts/build.sh

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Building RuVector WASM bindings...${NC}"

# Parse arguments
TARGET="bundler"
PROFILE="release"
FEATURES=""

while [[ $# -gt 0 ]]; do
  case $1 in
    --target)
      TARGET="$2"
      shift 2
      ;;
    --dev)
      PROFILE="dev"
      shift
      ;;
    --features)
      FEATURES="--features $2"
      shift 2
      ;;
    --all-targets)
      echo -e "${YELLOW}Building for all targets...${NC}"
      ./scripts/build.sh --target web --release
      ./scripts/build.sh --target bundler --release
      ./scripts/build.sh --target nodejs --release
      exit 0
      ;;
    *)
      echo -e "${RED}Unknown option: $1${NC}"
      exit 1
      ;;
  esac
done

# Build with wasm-pack
echo -e "${YELLOW}Target: $TARGET, Profile: $PROFILE${NC}"

if [ "$PROFILE" = "dev" ]; then
  wasm-pack build --dev --target $TARGET $FEATURES
else
  wasm-pack build --release --target $TARGET $FEATURES
fi

# Run TypeScript compilation
echo -e "${GREEN}Compiling TypeScript...${NC}"
npx tsc

# Copy additional files
echo -e "${GREEN}Copying assets...${NC}"
cp README.md pkg/ || true
cp LICENSE pkg/ || true

echo -e "${GREEN}Build complete!${NC}"
echo -e "Output directory: ${YELLOW}pkg/${NC}"
```

### 6.2 Test Script (scripts/test.sh)

```bash
#!/bin/bash
# scripts/test.sh

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}Running RuVector WASM tests...${NC}"

# Run WASM tests in browsers
echo -e "${YELLOW}Running WASM browser tests...${NC}"
wasm-pack test --headless --firefox --chrome

# Run Node.js tests
echo -e "${YELLOW}Running Node.js tests...${NC}"
wasm-pack test --node

# Run TypeScript/Jest tests
echo -e "${YELLOW}Running TypeScript tests...${NC}"
npm run test:js

echo -e "${GREEN}All tests passed!${NC}"
```

### 6.3 Release Script (scripts/release.sh)

```bash
#!/bin/bash
# scripts/release.sh

set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Check if version is provided
if [ -z "$1" ]; then
  echo -e "${RED}Usage: ./scripts/release.sh <version>${NC}"
  exit 1
fi

VERSION=$1

echo -e "${GREEN}Preparing release $VERSION...${NC}"

# Update version in Cargo.toml
echo -e "${YELLOW}Updating Cargo.toml...${NC}"
sed -i "s/^version = .*/version = \"$VERSION\"/" Cargo.toml

# Update version in package.json
echo -e "${YELLOW}Updating package.json...${NC}"
npm version $VERSION --no-git-tag-version

# Build all targets
echo -e "${YELLOW}Building all targets...${NC}"
./scripts/build.sh --all-targets

# Run all tests
echo -e "${YELLOW}Running tests...${NC}"
./scripts/test.sh

# Create git tag
echo -e "${YELLOW}Creating git tag...${NC}"
git add Cargo.toml package.json Cargo.lock
git commit -m "chore: Release v$VERSION"
git tag -a "v$VERSION" -m "Release version $VERSION"

echo -e "${GREEN}Release $VERSION ready!${NC}"
echo -e "${YELLOW}Next steps:${NC}"
echo "  1. git push origin main"
echo "  2. git push origin v$VERSION"
echo "  3. cd pkg && npm publish"
```

---

## 7. Examples

### 7.1 Browser Example (examples/browser/demo.ts)

```typescript
// examples/browser/demo.ts
import { ruVector, createTensor, AttentionConfig } from '../../js/index';

async function main() {
  // Initialize WASM
  console.log('Initializing RuVector WASM...');
  await ruVector.init();

  // Create attention config
  const config = new AttentionConfig(8, 64); // 8 heads, 64 dim per head
  config.set_dropout(0.1);

  // Create multi-head attention
  const attention = ruVector.createMultiHead(config);
  console.log('Created attention with', attention.num_parameters(), 'parameters');

  // Create sample data
  const batchSize = 2;
  const seqLen = 10;
  const modelDim = 512;

  const query = new Float32Array(batchSize * seqLen * modelDim);
  const key = new Float32Array(batchSize * seqLen * modelDim);
  const value = new Float32Array(batchSize * seqLen * modelDim);

  // Fill with random data
  for (let i = 0; i < query.length; i++) {
    query[i] = Math.random();
    key[i] = Math.random();
    value[i] = Math.random();
  }

  // Run attention
  console.log('Running attention...');
  const startTime = performance.now();

  const result = attention.forward(
    query,
    key,
    value,
    [batchSize, seqLen, seqLen, modelDim],
    undefined // no mask
  );

  const endTime = performance.now();
  console.log(`Attention computed in ${(endTime - startTime).toFixed(2)}ms`);

  // Display results
  const output = result.output;
  console.log('Output shape:', [batchSize, seqLen, modelDim]);
  console.log('Output sample:', output.slice(0, 10));

  // Memory usage
  const memUsage = ruVector.getMemoryUsage();
  console.log('Memory usage:', memUsage);
}

// Run on page load
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', main);
} else {
  main();
}
```

### 7.2 Node.js Example (examples/node/basic.mjs)

```javascript
// examples/node/basic.mjs
import { ruVector, AttentionConfig, HyperbolicConfig } from '@ruvector/attention-wasm';

async function demonstrateAttention() {
  // Initialize
  await ruVector.init();
  console.log('✓ RuVector initialized');

  // 1. Scaled Dot-Product Attention
  console.log('\n1. Scaled Dot-Product Attention');
  const scaledDot = ruVector.createScaledDotProduct(64, 0.0);

  const batch = 1;
  const seq = 5;
  const dim = 64;

  const q = new Float32Array(batch * seq * dim).fill(1.0);
  const k = new Float32Array(batch * seq * dim).fill(0.5);
  const v = new Float32Array(batch * seq * dim).fill(0.8);

  const output1 = scaledDot.forward(
    q, k, v,
    [batch, seq, dim],
    [batch, seq, dim],
    [batch, seq, dim],
    undefined
  );

  console.log(`  Output size: ${output1.length}`);
  console.log(`  Scale factor: ${scaledDot.scale}`);

  // 2. Multi-Head Attention
  console.log('\n2. Multi-Head Attention');
  const config = new AttentionConfig(8, 64);
  const multiHead = ruVector.createMultiHead(config);

  const output2 = multiHead.forward(
    q, k, v,
    [batch, seq, seq, dim * 8],
    undefined
  );

  console.log(`  Num parameters: ${multiHead.num_parameters()}`);
  console.log(`  Output available: ${output2.output !== undefined}`);

  // 3. Hyperbolic Attention
  console.log('\n3. Hyperbolic Attention');
  const hypConfig = new HyperbolicConfig(-1.0, 4);
  const hyperbolic = ruVector.createHyperbolic(hypConfig);

  const output3 = hyperbolic.forward(
    q, k, v,
    [batch, seq, dim]
  );

  console.log(`  Curvature: ${hyperbolic.curvature}`);
  console.log(`  Output size: ${output3.length}`);

  // 4. Linear Attention (O(n) complexity)
  console.log('\n4. Linear Attention');
  const linear = ruVector.createLinear(512, 256);

  const q4 = new Float32Array(batch * seq * 512).fill(1.0);
  const k4 = new Float32Array(batch * seq * 512).fill(0.5);
  const v4 = new Float32Array(batch * seq * 512).fill(0.8);

  const output4 = linear.forward(
    q4, k4, v4,
    [batch, seq, 512]
  );

  console.log(`  Feature dim: ${linear.feature_dim}`);
  console.log(`  Output size: ${output4.length}`);

  // Memory usage
  const mem = ruVector.getMemoryUsage();
  console.log(`\nMemory: ${(mem.total / 1024 / 1024).toFixed(2)} MB`);
}

demonstrateAttention().catch(console.error);
```

### 7.3 Performance Benchmark (examples/node/performance.mjs)

```javascript
// examples/node/performance.mjs
import { ruVector, AttentionConfig, PerfTimer } from '@ruvector/attention-wasm';

async function benchmark() {
  await ruVector.init();

  const config = new AttentionConfig(8, 64);
  const attention = ruVector.createMultiHead(config);

  const batchSizes = [1, 2, 4, 8];
  const seqLengths = [16, 32, 64, 128, 256];
  const modelDim = 512;

  console.log('Running performance benchmarks...\n');
  console.log('Batch | SeqLen | Time (ms) | Throughput (ops/sec)');
  console.log('------|--------|-----------|---------------------');

  for (const batch of batchSizes) {
    for (const seq of seqLengths) {
      const size = batch * seq * modelDim;
      const q = new Float32Array(size).fill(Math.random());
      const k = new Float32Array(size).fill(Math.random());
      const v = new Float32Array(size).fill(Math.random());

      // Warmup
      attention.forward(q, k, v, [batch, seq, seq, modelDim], undefined);

      // Benchmark
      const iterations = 10;
      const timer = new PerfTimer();

      for (let i = 0; i < iterations; i++) {
        attention.forward(q, k, v, [batch, seq, seq, modelDim], undefined);
      }

      const elapsed = timer.elapsed();
      const avgTime = elapsed / iterations;
      const throughput = 1000 / avgTime;

      console.log(
        `${batch.toString().padStart(5)} | ` +
        `${seq.toString().padStart(6)} | ` +
        `${avgTime.toFixed(2).padStart(9)} | ` +
        `${throughput.toFixed(2).padStart(19)}`
      );
    }
  }
}

benchmark().catch(console.error);
```

---

## 8. Testing Strategy

### 8.1 WASM Unit Tests (tests/wasm.rs)

```rust
// tests/wasm.rs
#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;
use ruvector_attention_wasm::*;
use js_sys::Float32Array;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_scaled_dot_product() {
    let attention = attention::WasmScaledDotProduct::new(64, None);

    let batch = 1;
    let seq = 4;
    let dim = 64;
    let size = batch * seq * dim;

    let q = Float32Array::new_with_length(size as u32);
    let k = Float32Array::new_with_length(size as u32);
    let v = Float32Array::new_with_length(size as u32);

    // Fill with test data
    for i in 0..size {
        q.set_index(i as u32, 1.0);
        k.set_index(i as u32, 0.5);
        v.set_index(i as u32, 0.8);
    }

    let output = attention.forward(
        &q, &k, &v,
        vec![batch, seq, dim],
        vec![batch, seq, dim],
        vec![batch, seq, dim],
        None,
    );

    assert!(output.is_ok());
    assert_eq!(output.unwrap().length(), size as u32);
}

#[wasm_bindgen_test]
fn test_multi_head_attention() {
    let config = types::AttentionConfig::new(4, 64);
    let mut attention = attention::WasmMultiHeadAttention::new(config).unwrap();

    let batch = 2;
    let seq = 8;
    let model_dim = 256;
    let size = batch * seq * model_dim;

    let q = Float32Array::new_with_length(size as u32);
    let k = Float32Array::new_with_length(size as u32);
    let v = Float32Array::new_with_length(size as u32);

    let result = attention.forward(
        &q, &k, &v,
        vec![batch, seq, seq, model_dim],
        None,
    );

    assert!(result.is_ok());
}

#[wasm_bindgen_test]
fn test_hyperbolic_attention() {
    let config = types::HyperbolicConfig::new(-1.0, 4);
    let attention = attention::WasmHyperbolicAttention::new(config).unwrap();

    assert_eq!(attention.curvature(), -1.0);
}

#[wasm_bindgen_test]
fn test_memory_usage() {
    let usage = memory_usage();
    assert!(!usage.is_undefined());
}

#[wasm_bindgen_test]
fn test_version() {
    let ver = version();
    assert!(!ver.is_empty());
}
```

### 8.2 TypeScript Integration Tests (tests/integration.test.ts)

```typescript
// tests/integration.test.ts
import { ruVector, AttentionConfig, HyperbolicConfig, createTensor } from '../js/index';

describe('RuVector WASM Integration Tests', () => {
  beforeAll(async () => {
    await ruVector.init();
  });

  describe('Scaled Dot-Product Attention', () => {
    it('should create and compute attention', () => {
      const attention = ruVector.createScaledDotProduct(64);
      expect(attention).toBeDefined();
      expect(attention.scale).toBeCloseTo(0.125, 3); // 1/sqrt(64)
    });

    it('should handle different sequence lengths', () => {
      const attention = ruVector.createScaledDotProduct(32);

      const batch = 1;
      const seqQ = 5;
      const seqK = 8;
      const dim = 32;

      const q = new Float32Array(batch * seqQ * dim).fill(1.0);
      const k = new Float32Array(batch * seqK * dim).fill(0.5);
      const v = new Float32Array(batch * seqK * dim).fill(0.8);

      const output = attention.forward(
        q, k, v,
        [batch, seqQ, dim],
        [batch, seqK, dim],
        [batch, seqK, dim],
        undefined
      );

      expect(output).toBeInstanceOf(Float32Array);
      expect(output.length).toBe(batch * seqQ * dim);
    });
  });

  describe('Multi-Head Attention', () => {
    it('should create with valid configuration', () => {
      const config = new AttentionConfig(8, 64);
      const attention = ruVector.createMultiHead(config);

      expect(attention.num_parameters()).toBeGreaterThan(0);
      expect(attention.config.num_heads).toBe(8);
      expect(attention.config.head_dim).toBe(64);
    });

    it('should compute attention with output and weights', () => {
      const config = new AttentionConfig(4, 32);
      const attention = ruVector.createMultiHead(config);

      const batch = 1;
      const seq = 6;
      const modelDim = 128;

      const q = new Float32Array(batch * seq * modelDim).fill(1.0);
      const k = new Float32Array(batch * seq * modelDim).fill(0.5);
      const v = new Float32Array(batch * seq * modelDim).fill(0.8);

      const result = attention.forward(
        q, k, v,
        [batch, seq, seq, modelDim],
        undefined
      );

      expect(result.output).toBeDefined();
      expect(result.output).toBeInstanceOf(Float32Array);
    });
  });

  describe('Hyperbolic Attention', () => {
    it('should create with valid curvature', () => {
      const config = new HyperbolicConfig(-1.0, 4);
      const attention = ruVector.createHyperbolic(config);

      expect(attention.curvature).toBe(-1.0);
    });

    it('should compute hyperbolic distances', () => {
      const config = new HyperbolicConfig(-1.0, 4);
      const attention = ruVector.createHyperbolic(config);

      const points = new Float32Array([0.1, 0.2, 0.3, 0.4]);
      const distances = attention.distance_matrix(points, points);

      expect(distances).toBeInstanceOf(Float32Array);
    });
  });

  describe('Linear Attention', () => {
    it('should create with feature dimension', () => {
      const attention = ruVector.createLinear(256, 128);
      expect(attention.feature_dim).toBe(128);
    });
  });

  describe('Performance', () => {
    it('should handle large tensors efficiently', () => {
      const config = new AttentionConfig(8, 64);
      const attention = ruVector.createMultiHead(config);

      const batch = 4;
      const seq = 128;
      const modelDim = 512;
      const size = batch * seq * modelDim;

      const q = new Float32Array(size);
      const k = new Float32Array(size);
      const v = new Float32Array(size);

      const start = performance.now();
      const result = attention.forward(
        q, k, v,
        [batch, seq, seq, modelDim],
        undefined
      );
      const elapsed = performance.now() - start;

      expect(result.output).toBeDefined();
      expect(elapsed).toBeLessThan(1000); // Should complete in < 1 second
    });
  });

  describe('Memory Management', () => {
    it('should report memory usage', () => {
      const usage = ruVector.getMemoryUsage();

      expect(usage.total).toBeGreaterThan(0);
      expect(usage.allocated).toBeGreaterThanOrEqual(0);
    });
  });
});
```

---

## 9. Documentation and README

### 9.1 WASM Package README

````markdown
# @ruvector/attention-wasm

WebAssembly bindings for RuVector's high-performance attention mechanisms.

## Features

- 🚀 **High Performance**: Rust-powered WASM for near-native speed
- 🧠 **Multiple Attention Types**: Scaled dot-product, multi-head, hyperbolic, linear, cross, flash
- 🌐 **Universal**: Works in browsers, Node.js, and Deno
- 📦 **Zero Dependencies**: Self-contained WASM module
- 🔒 **Type-Safe**: Full TypeScript support
- ⚡ **Async Support**: Promise-based API for non-blocking operations

## Installation

```bash
npm install @ruvector/attention-wasm
```

## Quick Start

```typescript
import { ruVector, AttentionConfig } from '@ruvector/attention-wasm';

// Initialize WASM module
await ruVector.init();

// Create multi-head attention
const config = new AttentionConfig(8, 64);
const attention = ruVector.createMultiHead(config);

// Prepare tensors
const batch = 2, seq = 10, dim = 512;
const query = new Float32Array(batch * seq * dim).fill(Math.random());
const key = new Float32Array(batch * seq * dim).fill(Math.random());
const value = new Float32Array(batch * seq * dim).fill(Math.random());

// Compute attention
const result = attention.forward(
  query, key, value,
  [batch, seq, seq, dim],
  undefined
);

console.log('Output:', result.output);
```

## API Reference

### Initialization

```typescript
await ruVector.init(): Promise<void>
```

### Attention Mechanisms

#### Scaled Dot-Product Attention
```typescript
ruVector.createScaledDotProduct(headDim: number, dropout?: number)
```

#### Multi-Head Attention
```typescript
const config = new AttentionConfig(numHeads, headDim);
ruVector.createMultiHead(config)
```

#### Hyperbolic Attention
```typescript
const config = new HyperbolicConfig(curvature, numHeads);
ruVector.createHyperbolic(config)
```

#### Linear Attention (O(n))
```typescript
ruVector.createLinear(modelDim: number, featureDim: number)
```

#### Cross Attention
```typescript
const config = new AttentionConfig(numHeads, headDim);
ruVector.createCross(config)
```

#### Flash Attention
```typescript
ruVector.createFlash(headDim: number, blockSize?: number)
```

## Examples

See [examples/](./examples/) directory for complete examples.

## License

MIT OR Apache-2.0
````

---

## 10. Implementation Checklist

### Phase 1: Core Setup ✅
- [ ] Create directory structure
- [ ] Configure Cargo.toml with all dependencies
- [ ] Set up package.json and TypeScript config
- [ ] Implement error handling module
- [ ] Implement type definitions module
- [ ] Implement utilities module

### Phase 2: Attention Bindings ✅
- [ ] Implement WasmScaledDotProduct
- [ ] Implement WasmMultiHeadAttention
- [ ] Implement WasmHyperbolicAttention
- [ ] Implement WasmLinearAttention
- [ ] Implement WasmCrossAttention
- [ ] Implement WasmFlashAttention
- [ ] Export all attention modules

### Phase 3: TypeScript Layer ✅
- [ ] Create main TypeScript entry point
- [ ] Implement type definitions
- [ ] Implement utility functions
- [ ] Create high-level wrapper API

### Phase 4: Async Operations ✅
- [ ] Implement Promise-based APIs
- [ ] Create async wrappers for attention
- [ ] Implement batch processing

### Phase 5: Build System ✅
- [ ] Create build.sh script
- [ ] Create test.sh script
- [ ] Create release.sh script
- [ ] Configure wasm-pack for multiple targets

### Phase 6: Examples ✅
- [ ] Browser basic example
- [ ] Browser worker example
- [ ] Node.js basic example
- [ ] Node.js performance benchmark

### Phase 7: Testing ✅
- [ ] WASM unit tests (wasm-bindgen-test)
- [ ] TypeScript integration tests (Jest)
- [ ] Performance benchmarks
- [ ] Browser compatibility tests

### Phase 8: Documentation ✅
- [ ] API documentation
- [ ] Usage examples
- [ ] Performance guide
- [ ] Migration guide from pure Rust

### Phase 9: Optimization ✅
- [ ] Profile WASM bundle size
- [ ] Optimize for size vs speed
- [ ] Implement zero-copy where possible
- [ ] Add SIMD support flags
- [ ] Memory usage optimization

### Phase 10: Release ✅
- [ ] Version management
- [ ] NPM package preparation
- [ ] CI/CD pipeline
- [ ] Publishing documentation

---

## 11. Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| **Bundle Size** | < 500 KB | Optimized with LTO and strip |
| **Init Time** | < 100ms | WASM module initialization |
| **Memory Overhead** | < 10% | Compared to native Rust |
| **Throughput** | > 80% native | Compared to Rust implementation |
| **Browser Support** | All modern browsers | Chrome, Firefox, Safari, Edge |

---

## 12. Dependencies and Versions

```toml
wasm-bindgen = "0.2.92"
wasm-bindgen-futures = "0.4.42"
js-sys = "0.3.69"
web-sys = "0.3.69"
serde = "1.0"
serde-wasm-bindgen = "0.6.5"
```

---

## Conclusion

This implementation plan provides a complete roadmap for creating production-ready WebAssembly bindings for RuVector's attention mechanisms. The bindings will enable seamless integration in web browsers and Node.js environments while maintaining high performance and type safety.

**Next Steps:**
1. Begin with Phase 1 (Core Setup)
2. Implement and test each phase sequentially
3. Gather performance metrics and optimize
4. Publish to NPM registry

**Success Criteria:**
- All tests passing (100% coverage target)
- Bundle size under 500 KB
- TypeScript types fully generated
- Examples working in all target environments
- Documentation complete and accurate
