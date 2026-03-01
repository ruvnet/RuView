# EXO-AI 2025 Build Documentation

## Overview

EXO-AI 2025 is a cognitive substrate implementation featuring hypergraph computation, temporal dynamics, federation protocols, and WebAssembly compilation capabilities.

## Project Structure

```
exo-ai-2025/
├── crates/
│   ├── exo-core/                    ✅ COMPILES
│   ├── exo-hypergraph/              ✅ COMPILES
│   ├── exo-federation/              ✅ COMPILES
│   ├── exo-wasm/                    ✅ COMPILES
│   ├── exo-manifold/                ❌ FAILS (burn-core bincode issue)
│   ├── exo-backend-classical/       ❌ FAILS (39 API mismatch errors)
│   ├── exo-node/                    ❌ FAILS (6 API mismatch errors)
│   └── exo-temporal/                ❌ FAILS (7 API mismatch errors)
├── docs/
├── tests/
├── benches/
└── Cargo.toml                       (workspace configuration)
```

## Dependencies

### System Requirements

- **Rust**: 1.75.0 or later
- **Cargo**: Latest stable
- **Platform**: Linux, macOS, or Windows
- **Architecture**: x86_64, aarch64

### Key Dependencies

- **ruvector-core**: Vector database and similarity search
- **ruvector-graph**: Hypergraph data structures and algorithms
- **tokio**: Async runtime
- **serde**: Serialization framework
- **petgraph**: Graph algorithms
- **burn**: Machine learning framework (0.14.0)
- **wasm-bindgen**: WebAssembly bindings

## Build Instructions

### 1. Clone and Setup

```bash
cd /home/user/ruvector/examples/exo-ai-2025
```

### 2. Check Workspace Configuration

The workspace is configured with:
- 8 member crates
- Shared dependency versions
- Custom build profiles (dev, release, bench, test)

### 3. Build Individual Crates (Successful)

```bash
# Core substrate implementation
cargo build -p exo-core

# Hypergraph computation
cargo build -p exo-hypergraph

# Federation protocol
cargo build -p exo-federation

# WebAssembly compilation
cargo build -p exo-wasm
```

### 4. Attempt Full Workspace Build (Currently Fails)

```bash
# This will fail due to known issues
cargo build --workspace
```

**Expected Result**: 53 compilation errors across 4 crates

## Build Profiles

### Development Profile

```toml
[profile.dev]
opt-level = 0
debug = true
debug-assertions = true
overflow-checks = true
incremental = true
```

**Usage**: `cargo build` (default)

### Release Profile

```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
debug = false
strip = true
```

**Usage**: `cargo build --release`

### Benchmark Profile

```toml
[profile.bench]
inherits = "release"
lto = true
codegen-units = 1
```

**Usage**: `cargo bench`

### Test Profile

```toml
[profile.test]
opt-level = 1
debug = true
```

**Usage**: `cargo test`

## Known Issues

### Critical Issues (Build Failures)

#### 1. burn-core Bincode Compatibility (exo-manifold)

**Error**: `cannot find function 'decode_borrowed_from_slice' in module 'bincode::serde'`

**Cause**: burn-core 0.14.0 expects bincode 1.3.x API but resolves to bincode 2.0.x

**Status**: BLOCKING - prevents exo-manifold compilation

**Workaround Attempted**: Cargo patch to force bincode 1.3 (failed - same source error)

**Recommended Fix**:
- Wait for burn-core 0.15.0 with bincode 2.0 support
- OR use git patch to custom burn-core fork
- OR temporarily exclude exo-manifold from workspace

#### 2. exo-backend-classical API Mismatches (39 errors)

**Errors**: Type mismatches between exo-core API and backend implementation

Key issues:
- `SearchResult` missing `id` field
- `Metadata` changed from HashMap to struct (no `insert` method)
- `Pattern` missing `id` and `salience` fields
- `SubstrateTime` expects `i64` but receives `u64`
- `Filter` has `conditions` field instead of `metadata`
- Various Option/unwrap type mismatches

**Status**: BLOCKING - requires API refactoring

**Recommended Fix**: Align exo-backend-classical with exo-core v0.1.0 API

#### 3. exo-temporal API Mismatches (7 errors)

**Errors**: Similar API compatibility issues with exo-core

Key issues:
- `SearchResult` structure mismatch
- `Metadata` type changes
- `Pattern` field mismatches

**Status**: BLOCKING

**Recommended Fix**: Update to match exo-core API changes

#### 4. exo-node API Mismatches (6 errors)

**Errors**: Trait implementation and API mismatches

**Status**: BLOCKING

**Recommended Fix**: Implement updated exo-core traits correctly

### Warnings (Non-Blocking)

- **ruvector-core**: 12 unused import warnings
- **ruvector-graph**: 81 warnings (mostly unused code and missing docs)
- **exo-federation**: 8 warnings (unused variables)
- **exo-hypergraph**: 2 warnings (unused variables)

These warnings do not prevent compilation but should be addressed for code quality.

## Platform Support Matrix

| Platform | Architecture | Status | Notes |
|----------|-------------|--------|-------|
| Linux | x86_64 | ✅ Partial | Core crates compile |
| Linux | aarch64 | ⚠️ Untested | Should work |
| macOS | x86_64 | ⚠️ Untested | Should work |
| macOS | arm64 | ⚠️ Untested | Should work |
| Windows | x86_64 | ⚠️ Untested | May need adjustments |
| WASM | wasm32 | 🚧 Partial | exo-wasm compiles |

## Testing

### Unit Tests (Partial)

```bash
# Test individual crates
cargo test -p exo-core
cargo test -p exo-hypergraph
cargo test -p exo-federation
cargo test -p exo-wasm

# Full workspace test (will fail)
cargo test --workspace
```

### Integration Tests

Integration tests are located in `tests/` but currently cannot run due to build failures.

## Benchmarking

Benchmarks are located in `benches/` but require successful compilation of all crates.

```bash
# When compilation issues are resolved
cargo bench --workspace
```

## Continuous Integration

### Pre-commit Checks

```bash
# Check compilation
cargo check --workspace

# Run tests
cargo test --workspace

# Check formatting
cargo fmt --all -- --check

# Run linter (if clippy available)
cargo clippy --workspace -- -D warnings
```

## Troubleshooting

### Issue: "profiles for the non root package will be ignored"

**Symptom**: Warnings about profiles in exo-wasm and exo-node

**Solution**: Remove `[profile.*]` sections from individual crate Cargo.toml files. Profiles should only be defined at workspace root.

### Issue: "cannot find function in bincode::serde"

**Symptom**: burn-core compilation failure

**Solution**: See Known Issues #1. This is a dependency compatibility issue requiring upstream fix.

### Issue: "method not found" or "field does not exist"

**Symptom**: exo-backend-classical, exo-node, exo-temporal failures

**Solution**: These crates were developed against an older exo-core API. Requires refactoring to match current API.

## Next Steps

### Immediate Actions Required

1. **Fix burn-core bincode issue**:
   - Patch to use burn-core from git with bincode 2.0 support
   - OR exclude exo-manifold until burn 0.15.0 release

2. **Refactor backend crates**:
   - Update exo-backend-classical to match exo-core v0.1.0 API
   - Update exo-temporal API usage
   - Update exo-node trait implementations

3. **Address warnings**:
   - Remove unused imports
   - Add missing documentation
   - Fix unused variable warnings

### Verification Steps

After fixes are applied:

```bash
# 1. Clean build
cargo clean

# 2. Check workspace
cargo check --workspace

# 3. Build workspace
cargo build --workspace

# 4. Run tests
cargo test --workspace

# 5. Release build
cargo build --workspace --release

# 6. Verify benches
cargo bench --workspace --no-run
```

## Additional Resources

- **Project Repository**: https://github.com/ruvnet/ruvector
- **Ruvector Documentation**: See main project docs
- **Architecture Documentation**: See `architecture/` directory
- **Specifications**: See `specs/` directory

## Support

For build issues or questions:
1. Check this document for known issues
2. Review validation report: `docs/VALIDATION_REPORT.md`
3. Check architecture docs: `architecture/`
4. File an issue with full build output

---

**Last Updated**: 2025-11-29
**Workspace Version**: 0.1.0
**Build Status**: ⚠️ PARTIAL (4/8 crates compile successfully)
