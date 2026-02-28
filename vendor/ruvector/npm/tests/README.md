# rUvector NPM Package Test Suite

Comprehensive test suite for all rUvector npm packages.

## Test Structure

```
tests/
├── unit/                    # Unit tests for individual packages
│   ├── core.test.js        # @ruvector/core tests
│   ├── wasm.test.js        # @ruvector/wasm tests
│   ├── ruvector.test.js    # ruvector main package tests
│   └── cli.test.js         # CLI tests
├── integration/             # Cross-package integration tests
│   └── cross-package.test.js
├── performance/             # Performance benchmarks
│   └── benchmarks.test.js
├── fixtures/                # Test data and fixtures
│   └── temp/               # Temporary test files (auto-cleaned)
├── run-all-tests.js        # Test runner script
├── test-results.json       # Latest test results
└── README.md               # This file
```

## Running Tests

### All Tests

```bash
# From npm/tests directory
node run-all-tests.js

# Or from npm root
npm test
```

### Unit Tests Only

```bash
node run-all-tests.js --only=unit
```

### Integration Tests Only

```bash
node run-all-tests.js --only=integration
```

### Performance Benchmarks

```bash
node run-all-tests.js --perf
```

### Individual Test Files

```bash
# Run specific test file
node --test unit/core.test.js
node --test unit/wasm.test.js
node --test unit/ruvector.test.js
node --test integration/cross-package.test.js
```

## Test Coverage

### @ruvector/core (Native Module)

- ✅ Platform detection (Linux, macOS, Windows)
- ✅ Architecture detection (x64, arm64)
- ✅ Native binding loading
- ✅ VectorDB creation with options
- ✅ Vector insertion (single and batch)
- ✅ Vector search with HNSW
- ✅ Vector deletion and retrieval
- ✅ Distance metrics (Cosine, Euclidean, etc.)
- ✅ HNSW configuration
- ✅ Quantization options
- ✅ Version and utility functions

### @ruvector/wasm (WebAssembly Module)

- ✅ WASM module loading in Node.js
- ✅ Environment detection
- ✅ VectorDB initialization
- ✅ Vector operations (insert, search, delete, get)
- ✅ Batch operations
- ✅ Metadata support
- ✅ Float32Array and Array support
- ✅ SIMD detection
- ✅ Browser vs Node.js compatibility

### ruvector (Main Package)

- ✅ Backend detection and loading
- ✅ Native vs WASM fallback
- ✅ Platform prioritization
- ✅ VectorIndex creation
- ✅ API consistency across backends
- ✅ Utils functions (cosine, euclidean, normalize)
- ✅ TypeScript type definitions
- ✅ Error handling
- ✅ Stats and optimization

### CLI (ruvector command)

- ✅ Command availability
- ✅ Help and version commands
- ✅ Info command (backend info)
- ✅ Init command (index creation)
- ✅ Insert command (batch insert)
- ✅ Search command
- ✅ Stats command
- ✅ Benchmark command
- ✅ Error handling
- ✅ Output formatting

### Integration Tests

- ✅ Backend loading consistency
- ✅ API compatibility between native/WASM
- ✅ Data consistency across operations
- ✅ Search result determinism
- ✅ Error handling consistency
- ✅ TypeScript types availability

### Performance Benchmarks

- ✅ Insert throughput (single and batch)
- ✅ Search latency and throughput
- ✅ Concurrent search performance
- ✅ Dimension scaling (128, 384, 768, 1536)
- ✅ Memory usage analysis
- ✅ Backend comparison
- ✅ Utils performance

## Expected Behavior

### Test Skipping

Tests automatically skip when dependencies are unavailable:

- **@ruvector/core tests**: Skipped if native bindings not built for current platform
- **@ruvector/wasm tests**: Skipped if WASM not built (`npm run build:wasm` required)
- **CLI tests**: Skipped if dependencies not installed

### Performance Expectations

Minimum performance targets (may vary by backend):

- **Insert**: >10 vectors/sec (single), >1000 vectors/sec (batch)
- **Search**: >5 queries/sec
- **Latency**: <1000ms average for k=10 searches
- **Memory**: <5KB per vector (with overhead)

## Test Results

After running tests, check `test-results.json` for detailed results:

```json
{
  "timestamp": "2024-01-01T00:00:00.000Z",
  "summary": {
    "total": 5,
    "passed": 5,
    "failed": 0,
    "passRate": "100.0%"
  },
  "results": [...]
}
```

## Prerequisites

### For @ruvector/core tests:

```bash
# Build native bindings (from project root)
cargo build --release
npm run build:napi
```

### For @ruvector/wasm tests:

```bash
# Build WASM (requires wasm-pack)
cd npm/wasm
npm run build:wasm
```

### For all tests:

```bash
# Install dependencies for each package
cd npm/core && npm install
cd npm/wasm && npm install
cd npm/ruvector && npm install
```

## Troubleshooting

### "Cannot find module" errors

- Ensure dependencies are installed: `npm install` in each package
- Build packages first: `npm run build` in each package

### "Native binding not available"

- Build Rust crates first: `cargo build --release`
- Check platform support: Currently supports linux-x64, darwin-arm64, etc.

### "WASM module not found"

- Build WASM: `cd npm/wasm && npm run build:wasm`
- Install wasm-pack: `cargo install wasm-pack`

### Tests timeout

- Increase timeout for performance tests
- Use `--perf` flag separately for benchmarks
- Run individual test files for debugging

## CI/CD Integration

Add to your CI pipeline:

```yaml
# .github/workflows/test.yml
- name: Run Tests
  run: |
    cd npm/tests
    node run-all-tests.js
```

## Contributing

When adding new features:

1. Add unit tests in `unit/`
2. Add integration tests if it affects multiple packages
3. Add performance benchmarks if it's performance-critical
4. Update this README with new test coverage
5. Ensure all tests pass before submitting PR

## License

MIT
