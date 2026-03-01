# ruvllm-wasm

[![Crates.io](https://img.shields.io/crates/v/ruvllm-wasm.svg)](https://crates.io/crates/ruvllm-wasm)
[![Documentation](https://docs.rs/ruvllm-wasm/badge.svg)](https://docs.rs/ruvllm-wasm)
[![License](https://img.shields.io/crates/l/ruvllm-wasm.svg)](https://github.com/ruvnet/ruvector/blob/main/LICENSE)

**WASM bindings for browser-based LLM inference** with WebGPU acceleration, SIMD optimizations, and intelligent routing.

## Features

- **WebGPU Acceleration** - 10-50x faster inference with GPU compute shaders
- **SIMD Optimizations** - Vectorized operations for CPU fallback
- **Web Workers** - Parallel inference without blocking the main thread
- **GGUF Support** - Load quantized models (Q4, Q5, Q8) for efficient browser inference
- **Streaming Tokens** - Real-time token generation for responsive UX
- **Intelligent Routing** - HNSW Router, MicroLoRA, SONA for optimized inference

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ruvllm-wasm = "2.0"
```

Or build for WASM:

```bash
wasm-pack build --target web --release
```

## Quick Start

```rust
use ruvllm_wasm::{RuvLLMWasm, GenerationConfig};

// Initialize with WebGPU (if available)
let llm = RuvLLMWasm::new(true).await?;

// Load a GGUF model
llm.load_model_from_url("https://example.com/model.gguf").await?;

// Generate text
let config = GenerationConfig {
    max_tokens: 100,
    temperature: 0.7,
    top_p: 0.9,
    ..Default::default()
};

let result = llm.generate("What is the capital of France?", &config).await?;
println!("{}", result.text);
```

## JavaScript Usage

```javascript
import init, { RuvLLMWasm } from 'ruvllm-wasm';

await init();

// Create instance with WebGPU
const llm = await RuvLLMWasm.new(true);

// Load model
await llm.load_model_from_url('https://example.com/model.gguf', (loaded, total) => {
  console.log(`Loading: ${Math.round(loaded / total * 100)}%`);
});

// Generate with streaming
await llm.generate_stream('Tell me a story', {
  max_tokens: 200,
  temperature: 0.8,
}, (token) => {
  process.stdout.write(token);
});
```

## Features

### WebGPU Acceleration

```toml
[dependencies]
ruvllm-wasm = { version = "2.0", features = ["webgpu"] }
```

Enables GPU-accelerated inference using WebGPU compute shaders:
- Matrix multiplication kernels
- Attention computation
- 10-50x speedup on supported browsers

### Parallel Inference

```toml
[dependencies]
ruvllm-wasm = { version = "2.0", features = ["parallel"] }
```

Run inference in Web Workers:
- Non-blocking main thread
- Multiple concurrent requests
- Automatic worker pool management

### SIMD Optimizations

```toml
[dependencies]
ruvllm-wasm = { version = "2.0", features = ["simd"] }
```

Requires building with SIMD target:
```bash
RUSTFLAGS="-C target-feature=+simd128" wasm-pack build --target web
```

### Intelligent Features

```toml
[dependencies]
ruvllm-wasm = { version = "2.0", features = ["intelligent"] }
```

Enables advanced AI features:
- **HNSW Router** - Semantic routing for multi-model deployments
- **MicroLoRA** - Lightweight adapter injection
- **SONA Instant** - Self-optimizing neural adaptation

## Browser Requirements

| Feature | Required | Benefit |
|---------|----------|---------|
| WebAssembly | Yes | Core execution |
| WebGPU | No (recommended) | 10-50x faster |
| SharedArrayBuffer | No | Multi-threading |
| SIMD | No | 2-4x faster math |

### Enable SharedArrayBuffer

Add these headers to your server:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

## Recommended Models

| Model | Size | Use Case |
|-------|------|----------|
| TinyLlama-1.1B-Q4 | ~700 MB | General chat |
| Phi-2-Q4 | ~1.6 GB | Code, reasoning |
| Qwen2-0.5B-Q4 | ~400 MB | Fast responses |
| StableLM-Zephyr-3B-Q4 | ~2 GB | Quality chat |

## API Reference

### RuvLLMWasm

```rust
impl RuvLLMWasm {
    /// Create a new instance
    pub async fn new(use_webgpu: bool) -> Result<Self, JsValue>;

    /// Load model from URL
    pub async fn load_model_from_url(&self, url: &str) -> Result<(), JsValue>;

    /// Load model from bytes
    pub async fn load_model_from_bytes(&self, bytes: &[u8]) -> Result<(), JsValue>;

    /// Generate text completion
    pub async fn generate(&self, prompt: &str, config: &GenerationConfig) -> Result<GenerationResult, JsValue>;

    /// Generate with streaming callback
    pub async fn generate_stream(&self, prompt: &str, config: &GenerationConfig, callback: js_sys::Function) -> Result<GenerationResult, JsValue>;

    /// Check WebGPU availability
    pub async fn check_webgpu() -> WebGPUStatus;

    /// Get browser capabilities
    pub async fn get_capabilities() -> BrowserCapabilities;

    /// Unload model and free memory
    pub fn unload(&self);
}
```

## Related Packages

- [ruvllm](https://crates.io/crates/ruvllm) - Core LLM runtime
- [ruvllm-cli](https://crates.io/crates/ruvllm-cli) - CLI for model inference
- [@ruvector/ruvllm-wasm](https://www.npmjs.com/package/@ruvector/ruvllm-wasm) - npm package

## License

MIT OR Apache-2.0

---

**Part of the [RuVector](https://github.com/ruvnet/ruvector) ecosystem** - High-performance vector database with self-learning capabilities.
