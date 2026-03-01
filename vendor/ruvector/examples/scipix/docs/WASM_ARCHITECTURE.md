# WebAssembly Architecture

## Overview

The Scipix WASM module provides browser-based OCR with LaTeX support through a carefully designed architecture optimizing for performance and developer experience.

## Module Structure

```
src/wasm/
├── mod.rs          # Module entry, initialization
├── api.rs          # JavaScript API surface
├── worker.rs       # Web Worker support
├── canvas.rs       # Canvas/ImageData handling
├── memory.rs       # Memory management
└── types.rs        # Type definitions

web/
├── index.js        # JavaScript wrapper
├── worker.js       # Worker thread script
├── types.ts        # TypeScript definitions
├── example.html    # Demo application
└── package.json    # NPM configuration
```

## Key Components

### 1. WASM Core (`mod.rs`)

Initializes the WASM module with:
- Panic hooks for better error messages
- Custom allocator (wee_alloc) for smaller binary
- Logging infrastructure

```rust
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
}
```

### 2. JavaScript API (`api.rs`)

Provides the main `ScipixWasm` class with methods:
- Image recognition from various sources
- Format configuration
- Batch processing
- Confidence filtering

Uses `wasm-bindgen` for seamless JS interop:

```rust
#[wasm_bindgen]
pub struct ScipixWasm { ... }

#[wasm_bindgen]
impl ScipixWasm {
    #[wasm_bindgen(constructor)]
    pub async fn new() -> Result<ScipixWasm, JsValue> { ... }
}
```

### 3. Web Worker Support (`worker.rs`)

Enables off-main-thread processing:
- Message-based communication
- Progress reporting
- Batch processing with updates

Worker flow:
```
Main Thread          Worker Thread
    │                     │
    ├──── Init ──────────>│
    │<──── Ready ─────────┤
    │                     │
    ├──── Process ───────>│
    │<──── Started ───────┤
    │<──── Progress ──────┤
    │<──── Success ───────┤
```

### 4. Canvas Processing (`canvas.rs`)

Handles browser-specific image sources:
- `HTMLCanvasElement` extraction
- `ImageData` conversion
- Blob URL loading
- Image preprocessing

```rust
pub fn extract_canvas_image(&self, canvas: &HtmlCanvasElement)
    -> Result<ImageData>
```

### 5. Memory Management (`memory.rs`)

Optimizes WASM memory usage:
- Efficient buffer allocation
- Memory pooling
- Automatic cleanup
- Shared memory support

```rust
pub struct WasmBuffer {
    data: Vec<u8>,
}

impl Drop for WasmBuffer {
    fn drop(&mut self) {
        self.data.clear();
        self.data.shrink_to_fit();
    }
}
```

## Build Pipeline

### Compilation

```bash
# Development build
wasm-pack build --target web --dev

# Production build
wasm-pack build --target web --release
```

### Optimizations

**Cargo.toml settings:**
```toml
[profile.release]
opt-level = "z"        # Optimize for size
lto = true             # Link-time optimization
codegen-units = 1      # Better optimization
strip = true           # Remove debug symbols
panic = "abort"        # Smaller panic handler
```

**Result:** ~800KB gzipped bundle

## Data Flow

### Main Thread Processing

```
Image File
    ↓
FileReader API
    ↓
Uint8Array
    ↓
WASM Memory
    ↓
Image Decode
    ↓
Preprocessing
    ↓
OCR Engine
    ↓
Result (JsValue)
    ↓
JavaScript
```

### Worker Thread Processing

```
Main Thread              Worker Thread
    │                         │
Image File                    │
    ↓                         │
Uint8Array                    │
    ├────────────────────────>│
    │                    WASM Memory
    │                         ↓
    │                   OCR Processing
    │                         ↓
    │<────────────────── Result
    ↓
Display
```

## Memory Layout

### WASM Linear Memory

```
┌─────────────────────┐
│   Stack             │ Growing down
├─────────────────────┤
│   ...               │
├─────────────────────┤
│   Image Buffers     │ Pool-allocated
├─────────────────────┤
│   Model Data        │ Static
├─────────────────────┤
│   Heap              │ Growing up
└─────────────────────┘
```

### Buffer Management

1. **Acquire** buffer from pool or allocate
2. **Process** image data
3. **Release** buffer back to pool
4. **Cleanup** on drop if pool is full

## Type Safety

### Rust → JavaScript

```rust
#[wasm_bindgen]
pub struct OcrResult {
    pub text: String,
    pub confidence: f32,
}
```

Generates:
```javascript
export class OcrResult {
  readonly text: string;
  readonly confidence: number;
}
```

### TypeScript Definitions

Manual definitions in `types.ts` provide:
- Full API documentation
- IntelliSense support
- Type checking
- Better DX

## Error Handling

### Rust Side

```rust
pub enum ScipixError {
    ImageProcessing(String),
    Ocr(String),
    InvalidInput(String),
}

impl From<ScipixError> for JsValue {
    fn from(error: ScipixError) -> Self {
        JsValue::from_str(&error.to_string())
    }
}
```

### JavaScript Side

```javascript
try {
    const result = await scipix.recognize(imageData);
} catch (error) {
    console.error('OCR failed:', error.message);
}
```

## Performance Considerations

### 1. Initialization

- **Lazy loading**: Only load WASM when needed
- **Caching**: Reuse instances
- **Singleton pattern**: One shared processor

### 2. Processing

- **Streaming**: Process images as they arrive
- **Workers**: Parallel processing
- **Batching**: Group similar operations

### 3. Memory

- **Pooling**: Reuse buffers
- **Cleanup**: Explicit disposal
- **Monitoring**: Track usage

### 4. Network

- **Compression**: Gzip WASM module
- **CDN**: Cache static assets
- **Prefetch**: Load before needed

## Browser Compatibility

### Required Features

- ✅ WebAssembly (97% global support)
- ✅ ES6 Modules (96% global support)
- ✅ Async/Await (96% global support)
- ⚠️ Web Workers (optional, 97% support)
- ⚠️ SharedArrayBuffer (optional, 92% support)

### Polyfills

Not required for core functionality. Workers are progressive enhancement.

## Security

### Content Security Policy

```html
<meta http-equiv="Content-Security-Policy"
      content="script-src 'self' 'wasm-unsafe-eval'">
```

### Sandboxing

WASM runs in browser sandbox:
- No file system access
- No network access (from WASM)
- Memory isolation

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    async fn test_recognition() {
        // Test WASM functions
    }
}
```

Run with:
```bash
wasm-pack test --headless --firefox
```

### Integration Tests

JavaScript tests using the built module:
```javascript
import { createScipix } from './index.js';

test('recognizes text', async () => {
    const scipix = await createScipix();
    const result = await scipix.recognize(testImage);
    expect(result.text).toBeTruthy();
});
```

## Debugging

### Development Mode

```bash
RUST_LOG=debug wasm-pack build --dev
```

### Browser DevTools

- Console logging via `tracing_wasm`
- Memory profiling
- Performance timeline
- Network inspection

### Source Maps

Enabled in dev builds for Rust source debugging.

## Future Enhancements

1. **Streaming OCR**: Process video frames
2. **Model loading**: Dynamic ONNX models
3. **Caching**: IndexedDB for results
4. **PWA**: Offline support
5. **SIMD**: Use WebAssembly SIMD
6. **Threads**: SharedArrayBuffer parallelism

## References

- [wasm-bindgen Guide](https://rustwasm.github.io/wasm-bindgen/)
- [web-sys Documentation](https://rustwasm.github.io/wasm-bindgen/api/web_sys/)
- [WebAssembly Spec](https://webassembly.github.io/spec/)
- [MDN WebAssembly](https://developer.mozilla.org/en-US/docs/WebAssembly)
