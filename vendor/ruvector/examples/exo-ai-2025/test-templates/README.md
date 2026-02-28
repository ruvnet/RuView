# EXO-AI 2025 Test Templates

## Purpose

This directory contains comprehensive test templates for all EXO-AI 2025 crates. These templates are ready to be copied into the actual crate directories once the implementation code is written.

## Directory Structure

```
test-templates/
├── exo-core/
│   └── tests/
│       └── core_traits_test.rs        # Core trait and type tests
├── exo-manifold/
│   └── tests/
│       └── manifold_engine_test.rs    # Manifold engine tests
├── exo-hypergraph/
│   └── tests/
│       └── hypergraph_test.rs         # Hypergraph substrate tests
├── exo-temporal/
│   └── tests/
│       └── temporal_memory_test.rs    # Temporal memory tests
├── exo-federation/
│   └── tests/
│       └── federation_test.rs         # Federation and consensus tests
├── exo-backend-classical/
│   └── tests/
│       └── classical_backend_test.rs  # ruvector integration tests
├── integration/
│   ├── manifold_hypergraph_test.rs    # Cross-crate integration
│   ├── temporal_federation_test.rs    # Distributed memory
│   └── full_stack_test.rs             # Complete system tests
└── README.md                           # This file
```

## How to Use

### 1. When Crates Are Created

Once a coder agent creates a crate (e.g., `crates/exo-core/`), copy the corresponding test template:

```bash
# Example for exo-core
cp test-templates/exo-core/tests/core_traits_test.rs \
   crates/exo-core/tests/

# Uncomment the use statements and imports
# Remove placeholder comments
# Run tests
cd crates/exo-core
cargo test
```

### 2. Activation Checklist

For each test file:
- [ ] Copy to actual crate directory
- [ ] Uncomment `use` statements
- [ ] Remove placeholder comments
- [ ] Add `#[cfg(test)]` if not present
- [ ] Run `cargo test` to verify
- [ ] Fix any compilation errors
- [ ] Ensure tests pass or fail appropriately (TDD)

### 3. Test Categories Covered

Each crate has tests for:

#### exo-core
- ✅ Pattern construction and validation
- ✅ TopologicalQuery variants
- ✅ SubstrateTime operations
- ✅ Error handling
- ✅ Filter types

#### exo-manifold
- ✅ Gradient descent retrieval
- ✅ Manifold deformation
- ✅ Strategic forgetting
- ✅ SIREN network operations
- ✅ Fourier features
- ✅ Tensor Train compression (feature-gated)
- ✅ Edge cases (NaN, infinity, etc.)

#### exo-hypergraph
- ✅ Hyperedge creation and query
- ✅ Persistent homology (0D, 1D, 2D)
- ✅ Betti numbers
- ✅ Sheaf consistency (feature-gated)
- ✅ Simplicial complex operations
- ✅ Entity and relation indexing

#### exo-temporal
- ✅ Causal cone queries (past, future, light-cone)
- ✅ Memory consolidation
- ✅ Salience computation
- ✅ Anticipatory pre-fetch
- ✅ Causal graph operations
- ✅ Temporal knowledge graph
- ✅ Short-term buffer management

#### exo-federation
- ✅ Post-quantum key exchange (Kyber)
- ✅ Byzantine fault tolerance
- ✅ CRDT reconciliation
- ✅ Onion routing
- ✅ Federation handshake
- ✅ Raft consensus
- ✅ Encrypted channels

#### exo-backend-classical
- ✅ ruvector-core integration
- ✅ ruvector-graph integration
- ✅ ruvector-gnn integration
- ✅ SubstrateBackend implementation
- ✅ Performance tests
- ✅ Concurrency tests

### 4. Integration Tests

Integration tests in `integration/` should be placed in `crates/tests/` at the workspace root:

```bash
# Create workspace integration test directory
mkdir -p crates/tests

# Copy integration tests
cp test-templates/integration/*.rs crates/tests/
```

### 5. Running Tests

```bash
# Run all tests in workspace
cargo test --all-features

# Run tests for specific crate
cargo test -p exo-manifold

# Run specific test file
cargo test -p exo-manifold --test manifold_engine_test

# Run with coverage
cargo tarpaulin --all-features

# Run integration tests only
cargo test --test '*'

# Run benchmarks
cargo bench
```

### 6. Test-Driven Development Workflow

1. **Copy template** to crate directory
2. **Uncomment imports** and test code
3. **Run tests** - they will fail (RED)
4. **Implement code** to make tests pass
5. **Run tests** again - they should pass (GREEN)
6. **Refactor** code while keeping tests green
7. **Repeat** for next test

### 7. Feature Gates

Some tests are feature-gated:

```rust
#[test]
#[cfg(feature = "tensor-train")]
fn test_tensor_train_compression() {
    // Only runs with --features tensor-train
}

#[test]
#[cfg(feature = "sheaf-consistency")]
fn test_sheaf_consistency() {
    // Only runs with --features sheaf-consistency
}

#[test]
#[cfg(feature = "post-quantum")]
fn test_kyber_key_exchange() {
    // Only runs with --features post-quantum
}
```

Run with features:
```bash
cargo test --features tensor-train
cargo test --all-features
```

### 8. Async Tests

Federation and temporal tests use `tokio::test`:

```rust
#[tokio::test]
async fn test_async_operation() {
    // Async test code
}
```

Ensure `tokio` is in dev-dependencies:
```toml
[dev-dependencies]
tokio = { version = "1.0", features = ["full", "test-util"] }
```

### 9. Test Data and Fixtures

Common test utilities should be placed in:
```
crates/test-utils/
├── src/
│   ├── fixtures.rs    # Test data generators
│   ├── mocks.rs       # Mock implementations
│   └── helpers.rs     # Test helper functions
```

### 10. Coverage Reports

Generate coverage reports:

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage
cargo tarpaulin --all-features --out Html --output-dir coverage/

# View report
open coverage/index.html
```

### 11. Continuous Integration

Tests should be run in CI:

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --all-features
      - run: cargo test --test '*'  # Integration tests
```

## Test Metrics

### Coverage Targets
- **Unit Tests**: 85%+ statement coverage
- **Integration Tests**: 70%+ coverage
- **E2E Tests**: Key user scenarios

### Performance Targets
| Operation | Target Latency | Target Throughput |
|-----------|----------------|-------------------|
| Manifold Retrieve (k=10) | <10ms | >1000 qps |
| Hyperedge Creation | <1ms | >10000 ops/s |
| Causal Query | <20ms | >500 qps |
| Byzantine Commit | <100ms | >100 commits/s |

## Next Steps

1. ✅ **Test strategy created** (`docs/TEST_STRATEGY.md`)
2. ✅ **Test templates created** (this directory)
3. ⏳ **Wait for coder to create crates**
4. ⏳ **Copy templates to crates**
5. ⏳ **Uncomment and activate tests**
6. ⏳ **Run tests (TDD: RED phase)**
7. ⏳ **Implement code to pass tests**
8. ⏳ **Achieve GREEN phase**
9. ⏳ **Refactor and optimize**

## References

- **Test Strategy**: `../docs/TEST_STRATEGY.md`
- **Architecture**: `../architecture/ARCHITECTURE.md`
- **Specification**: `../specs/SPECIFICATION.md`
- **Pseudocode**: `../architecture/PSEUDOCODE.md`

## Contact

For questions about test implementation:
- Check `docs/TEST_STRATEGY.md` for comprehensive guidance
- Review template files for examples
- Ensure TDD workflow is followed
