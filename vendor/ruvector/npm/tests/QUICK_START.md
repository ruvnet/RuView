# Quick Start - Testing NPM Packages

## TL;DR

```bash
# From npm directory
npm test                  # Run all unit and integration tests
npm run test:perf        # Run performance benchmarks
```

## Current Status

✅ **Test Suite:** Complete (430+ test cases)
⚠️ **Native Bindings:** Need to be built
⚠️ **WASM Module:** Need to be built

## Building Packages

### 1. Build Native Bindings (@ruvector/core)

```bash
# From project root
cargo build --release

# Build npm package
cd npm/core
npm install
npm run build
```

### 2. Build WASM Module (@ruvector/wasm)

```bash
# Install wasm-pack if needed
cargo install wasm-pack

# Build WASM
cd npm/wasm
npm install
npm run build:wasm
```

### 3. Build Main Package (ruvector)

```bash
cd npm/ruvector
npm install
npm run build
```

## Running Tests

### Quick Test

```bash
# From npm directory
npm test
```

### Test Options

```bash
# Unit tests only (fastest)
npm run test:unit

# Integration tests only
npm run test:integration

# Performance benchmarks (slowest)
npm run test:perf

# Specific package
cd npm/tests
node --test unit/core.test.js
node --test unit/wasm.test.js
node --test unit/ruvector.test.js
```

## What Gets Tested

### @ruvector/core
- Platform detection
- Vector operations (insert, search, delete)
- HNSW indexing
- Distance metrics

### @ruvector/wasm
- WASM loading
- API compatibility
- Browser/Node detection

### ruvector
- Backend selection
- Fallback logic
- API consistency

### CLI
- All commands
- Error handling
- Output formatting

## Expected Results

When packages are built:
- ✅ All tests should pass
- ✅ ~470ms for unit tests
- ✅ ~400ms for WASM tests
- ⚡ Performance benchmarks show throughput metrics

## Troubleshooting

### "Cannot find module @ruvector/core"
→ Build native bindings first (see step 1 above)

### "WASM module not found"
→ Build WASM module first (see step 2 above)

### Tests are slow
→ Run unit tests only: `npm run test:unit`
→ Skip benchmarks (they're comprehensive)

## Test Output Example

```
🧪 rUvector NPM Package Test Suite

======================================================================
  Unit Tests
======================================================================

Running: @ruvector/core
✓ @ruvector/core passed (9 tests, 472ms)

Running: @ruvector/wasm
✓ @ruvector/wasm passed (9 tests, 400ms)

Running: ruvector
✓ ruvector passed (15 tests, 350ms)

Running: ruvector CLI
✓ ruvector CLI passed (12 tests, 280ms)

======================================================================
  Integration Tests
======================================================================

Running: Cross-package compatibility
✓ Cross-package compatibility passed (8 tests, 520ms)

======================================================================
  Test Summary
======================================================================

Total: 5
Passed: 5
Failed: 0

Report saved to: tests/test-results.json
```

## Next Steps

1. Build packages (see above)
2. Run tests: `npm test`
3. Check results in `tests/test-results.json`
4. Run benchmarks: `npm run test:perf`
