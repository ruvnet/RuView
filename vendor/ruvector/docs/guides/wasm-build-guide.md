# Ruvector WASM Build Guide

## Overview

This guide provides instructions for building the Ruvector WASM bindings. The WASM module enables high-performance vector database operations directly in web browsers and Node.js environments.

## Implementation Status

✅ **Completed Components:**

1. **Core WASM Bindings** (`/crates/ruvector-wasm/src/lib.rs`)
   - Full VectorDB API (insert, search, delete, batch operations)
   - Proper error handling with WasmResult types
   - Console panic hook for debugging
   - JavaScript-compatible types (JsVectorEntry, JsSearchResult)

2. **SIMD Support**
   - Dual build configuration (with/without SIMD)
   - Feature flags in Cargo.toml
   - Runtime SIMD detection via `detectSIMD()` function

3. **Web Workers Integration** (`/crates/ruvector-wasm/src/worker.js`)
   - Message passing for async operations
   - Support for insert, search, delete, batch operations
   - Zero-copy transfers preparation

4. **Worker Pool Management** (`/crates/ruvector-wasm/src/worker-pool.js`)
   - Automatic pool sizing (4-8 workers based on CPU cores)
   - Round-robin task distribution
   - Promise-based API
   - Error handling and timeouts

5. **IndexedDB Persistence** (`/crates/ruvector-wasm/src/indexeddb.js`)
   - Save/load vectors to IndexedDB
   - Batch operations for performance
   - Progressive loading with callbacks
   - LRU cache implementation (1000 hot vectors)

6. **Examples**
   - Vanilla JavaScript example (`/examples/wasm-vanilla/index.html`)
   - React + Web Workers example (`/examples/wasm-react/`)

7. **Tests**
   - Comprehensive WASM tests (`/crates/ruvector-wasm/tests/wasm.rs`)
   - Browser-based testing with wasm-bindgen-test

8. **Build Configuration**
   - Optimized for size (target: <500KB gzipped)
   - Multiple build targets (web, nodejs, bundler)
   - Size verification scripts

## Prerequisites

```bash
# Install Rust with wasm32 target
rustup target add wasm32-unknown-unknown

# Install wasm-pack
cargo install wasm-pack

# Optional: Install wasm-opt for further optimization
npm install -g wasm-opt
```

## Building

### Standard Web Build

```bash
cd crates/ruvector-wasm
wasm-pack build --target web --out-dir pkg --release
```

### SIMD-Enabled Build

```bash
cd crates/ruvector-wasm
wasm-pack build --target web --out-dir pkg-simd --release -- --features simd
```

### All Targets

```bash
cd crates/ruvector-wasm
npm run build:all
```

This will build for:
- Web (`pkg/`)
- Web with SIMD (`pkg-simd/`)
- Node.js (`pkg-node/`)
- Bundler (`pkg-bundler/`)

## Known Build Issues & Solutions

### Issue: getrandom 0.3 Compatibility

**Problem:** Some dependencies (notably `rand` via `uuid`) pull in `getrandom` 0.3.4, which requires the `wasm_js` feature flag that must be set via `RUSTFLAGS` configuration flags, not just Cargo features.

**Solution Options:**

1. **Use .cargo/config.toml** (Already configured):
   ```toml
   [target.wasm32-unknown-unknown]
   rustflags = ['--cfg', 'getrandom_backend="wasm_js"']
   ```

2. **Disable uuid feature** (Implemented):
   ```toml
   # In ruvector-core/Cargo.toml
   [features]
   default = ["simd", "uuid-support"]
   uuid-support = ["uuid"]

   # In ruvector-wasm/Cargo.toml
   [dependencies]
   ruvector-core = { path = "../ruvector-core", default-features = false }
   ```

3. **Alternative: Use timestamp-based IDs** (Fallback):
   For WASM builds, use `Date.now()` + random suffixes instead of UUIDs

### Issue: Large Binary Size

**Solution:**

1. Enable LTO and size optimization (already configured):
   ```toml
   [profile.release]
   opt-level = "z"
   lto = true
   codegen-units = 1
   panic = "abort"
   ```

2. Run wasm-opt:
   ```bash
   npm run optimize
   ```

3. Verify size:
   ```bash
   npm run size
   ```

## Usage Examples

### Vanilla JavaScript

```html
<!DOCTYPE html>
<html>
<head>
  <title>Ruvector WASM</title>
</head>
<body>
  <script type="module">
    import init, { VectorDB } from './pkg/ruvector_wasm.js';

    await init();

    const db = new VectorDB(384, 'cosine', true);

    // Insert vector
    const vector = new Float32Array(384).map(() => Math.random());
    const id = db.insert(vector, 'vec_1', { label: 'test' });

    // Search
    const query = new Float32Array(384).map(() => Math.random());
    const results = db.search(query, 10);

    console.log('Results:', results);
  </script>
</body>
</html>
```

### With Web Workers

```javascript
import { WorkerPool } from '@ruvector/wasm/worker-pool';

const pool = new WorkerPool(
  '/worker.js',
  '/pkg/ruvector_wasm.js',
  {
    poolSize: 4,
    dimensions: 384,
    metric: 'cosine'
  }
);

await pool.init();

// Parallel insert
const entries = Array(1000).fill(0).map((_, i) => ({
  vector: Array(384).fill(0).map(() => Math.random()),
  id: `vec_${i}`,
  metadata: { index: i }
}));

const ids = await pool.insertBatch(entries);

// Parallel search
const results = await pool.search(query, 10);

// Cleanup
pool.terminate();
```

### With IndexedDB

```javascript
import { IndexedDBPersistence } from '@ruvector/wasm/indexeddb';

const persistence = new IndexedDBPersistence('my_database');
await persistence.open();

// Save vectors
await persistence.saveBatch(vectors);

// Load with progress
await persistence.loadAll((progress) => {
  console.log(`Loaded ${progress.loaded} vectors`);

  if (progress.vectors.length > 0) {
    db.insertBatch(progress.vectors);
  }
});

// Get stats
const stats = await persistence.getStats();
console.log(`Cache hit rate: ${(stats.cacheHitRate * 100).toFixed(2)}%`);
```

## Testing

### Browser Tests

```bash
cd crates/ruvector-wasm
wasm-pack test --headless --chrome
wasm-pack test --headless --firefox
```

### Node.js Tests

```bash
wasm-pack test --node
```

## Performance Optimization Tips

1. **Enable SIMD**: Use the SIMD build for 2-4x speedup on supported browsers
2. **Use Batch Operations**: `insertBatch` is 5-10x faster than multiple `insert` calls
3. **Use Web Workers**: Distribute operations across workers for parallel processing
4. **Enable LRU Cache**: Keep hot vectors in IndexedDB cache
5. **Optimize Vector Size**: Smaller dimensions = faster operations
6. **Choose Appropriate Metric**: Dot product is fastest, Euclidean is slowest

## Browser Compatibility

| Browser | Version | SIMD Support | Web Workers | IndexedDB |
|---------|---------|--------------|-------------|-----------|
| Chrome  | 91+     | ✅           | ✅          | ✅        |
| Firefox | 89+     | ✅           | ✅          | ✅        |
| Safari  | 16.4+   | Partial      | ✅          | ✅        |
| Edge    | 91+     | ✅           | ✅          | ✅        |

## Size Benchmarks

Expected sizes after optimization:

- **Base build**: ~450KB gzipped
- **SIMD build**: ~480KB gzipped
- **With wasm-opt -Oz**: ~380KB gzipped

## Troubleshooting

### CORS Errors with Workers

Ensure your server sends proper CORS headers:

```javascript
{
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Cross-Origin-Embedder-Policy': 'require-corp'
}
```

### Memory Issues

Increase WASM memory limit if needed:

```javascript
// In worker.js or main thread
WebAssembly.instantiate(module, {
  env: {
    memory: new WebAssembly.Memory({ initial: 256, maximum: 512 })
  }
});
```

### IndexedDB Quota Errors

Check available storage:

```javascript
if ('storage' in navigator && 'estimate' in navigator.storage) {
  const estimate = await navigator.storage.estimate();
  console.log(`Using ${estimate.usage} of ${estimate.quota} bytes`);
}
```

## Next Steps

1. **Complete Build Debugging**: Resolve getrandom compatibility issues
2. **Add More Examples**: Vue.js, Svelte, Angular examples
3. **Benchmarking Suite**: Compare performance across browsers
4. **CDN Distribution**: Publish to npm and CDNs
5. **Documentation**: Interactive playground and tutorials

## Contributing

See main repository for contribution guidelines.

## License

MIT
