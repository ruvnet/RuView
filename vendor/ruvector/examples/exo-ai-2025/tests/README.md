# EXO-AI 2025 Integration Tests

This directory contains comprehensive integration tests for the cognitive substrate platform.

## Test Organization

### Test Files

- **`substrate_integration.rs`** - Complete substrate workflow tests
  - Pattern storage and retrieval
  - Manifold deformation
  - Strategic forgetting
  - Bulk operations
  - Filtered queries

- **`hypergraph_integration.rs`** - Hypergraph substrate tests
  - Hyperedge creation and querying
  - Persistent homology computation
  - Betti number calculation
  - Sheaf consistency checking
  - Complex relational queries

- **`temporal_integration.rs`** - Temporal memory coordinator tests
  - Causal storage and queries
  - Light-cone constraints
  - Memory consolidation
  - Predictive anticipation
  - Temporal knowledge graphs

- **`federation_integration.rs`** - Federated mesh tests
  - CRDT merge operations
  - Byzantine consensus
  - Post-quantum handshakes
  - Onion-routed queries
  - Network partition tolerance

### Test Utilities

The `common/` directory contains shared testing infrastructure:

- **`fixtures.rs`** - Test data generators and builders
- **`assertions.rs`** - Domain-specific assertion functions
- **`helpers.rs`** - Utility functions for testing

## Running Tests

### Quick Start

```bash
# Run all tests (currently all ignored until crates implemented)
cargo test --workspace

# Run tests with output
cargo test --workspace -- --nocapture

# Run specific test file
cargo test --test substrate_integration

# Run tests matching a pattern
cargo test causal
```

### Using the Test Runner Script

```bash
# Standard test run
./scripts/run-integration-tests.sh

# Verbose output
./scripts/run-integration-tests.sh --verbose

# Parallel execution
./scripts/run-integration-tests.sh --parallel

# Generate coverage report
./scripts/run-integration-tests.sh --coverage

# Run specific tests
./scripts/run-integration-tests.sh --filter "causal"
```

## Test-Driven Development (TDD) Workflow

These integration tests are written **BEFORE** implementation to define expected behavior.

### Current State

All tests are marked with `#[ignore]` because the crates don't exist yet.

### Implementation Workflow

1. **Implementer selects a test** (e.g., `test_substrate_store_and_retrieve`)
2. **Reads the test to understand requirements**
3. **Implements the crate to satisfy the test**
4. **Removes `#[ignore]` from the test**
5. **Runs `cargo test` to verify**
6. **Iterates until test passes**

### Example: Implementing Substrate Storage

```rust
// 1. Read the test in substrate_integration.rs
#[tokio::test]
#[ignore] // <- Remove this line when implementing
async fn test_substrate_store_and_retrieve() {
    // The test shows expected API:
    let config = SubstrateConfig::default();
    let backend = ClassicalBackend::new(config).unwrap();
    // ... etc
}

// 2. Implement exo-core and exo-backend-classical to match

// 3. Remove #[ignore] and run:
cargo test --test substrate_integration

// 4. Iterate until passing
```

## Test Requirements for Implementers

### exo-core

**Required types:**
- `Pattern` - Pattern with embedding, metadata, timestamp, antecedents
- `Query` - Query specification
- `SubstrateConfig` - Configuration
- `SearchResult` - Search result with score
- `SubstrateBackend` trait - Backend abstraction
- `TemporalContext` trait - Temporal operations

**Expected methods:**
- `SubstrateInstance::new(backend)` - Create substrate
- `substrate.store(pattern)` - Store pattern
- `substrate.search(query, k)` - Similarity search

### exo-manifold

**Required types:**
- `ManifoldEngine` - Learned manifold storage
- `ManifoldDelta` - Deformation result

**Expected methods:**
- `ManifoldEngine::new(config)` - Initialize
- `manifold.retrieve(tensor, k)` - Gradient descent retrieval
- `manifold.deform(pattern, salience)` - Continuous deformation
- `manifold.forget(region, decay_rate)` - Strategic forgetting

### exo-hypergraph

**Required types:**
- `HypergraphSubstrate` - Hypergraph storage
- `Hyperedge` - Multi-entity relationship
- `TopologicalQuery` - Topology query spec
- `PersistenceDiagram` - Homology results

**Expected methods:**
- `hypergraph.create_hyperedge(entities, relation)` - Create hyperedge
- `hypergraph.persistent_homology(dim, range)` - Compute persistence
- `hypergraph.betti_numbers(max_dim)` - Topological invariants
- `hypergraph.check_sheaf_consistency(sections)` - Sheaf check

### exo-temporal

**Required types:**
- `TemporalMemory` - Temporal coordinator
- `CausalConeType` - Cone specification
- `CausalResult` - Result with causal metadata
- `AnticipationHint` - Pre-fetch hint

**Expected methods:**
- `temporal.store(pattern, antecedents)` - Store with causality
- `temporal.causal_query(query, time, cone)` - Causal retrieval
- `temporal.consolidate()` - Short-term to long-term
- `temporal.anticipate(hints)` - Pre-fetch

### exo-federation

**Required types:**
- `FederatedMesh` - Federation coordinator
- `FederationScope` - Query scope
- `StateUpdate` - CRDT update
- `CommitProof` - Consensus proof

**Expected methods:**
- `mesh.join_federation(peer)` - Federation handshake
- `mesh.federated_query(query, scope)` - Distributed query
- `mesh.byzantine_commit(update)` - Consensus
- `mesh.merge_crdt_state(state)` - CRDT reconciliation

## Performance Targets

Integration tests should verify these performance characteristics:

| Operation | Target Latency | Notes |
|-----------|----------------|-------|
| Pattern storage | < 1ms | Classical backend |
| Similarity search (k=10) | < 10ms | 10K patterns |
| Manifold deformation | < 100ms | Single pattern |
| Hypergraph query | < 50ms | 1K entities |
| Causal query | < 20ms | 10K temporal patterns |
| CRDT merge | < 5ms | 100 operations |
| Consensus round | < 200ms | 4 nodes, no faults |

## Test Coverage Goals

- **Statement coverage**: > 80%
- **Branch coverage**: > 75%
- **Function coverage**: > 80%

Run with coverage:
```bash
cargo tarpaulin --workspace --out Html --output-dir coverage
```

## Debugging Failed Tests

### Enable Logging

```bash
RUST_LOG=debug cargo test --test substrate_integration -- --nocapture
```

### Run Single Test

```bash
cargo test --test substrate_integration test_substrate_store_and_retrieve -- --nocapture
```

### Use Test Helpers

```rust
use common::helpers::*;

init_test_logger(); // Enable logging in test

let (result, duration) = measure_async(async {
    substrate.search(query, 10).await
}).await;

println!("Query took {:?}", duration);
```

## Contributing Tests

When adding new integration tests:

1. **Follow existing patterns** - Use the same structure as current tests
2. **Use test utilities** - Leverage `common/` helpers
3. **Document expectations** - Comment expected behavior clearly
4. **Mark as ignored** - Add `#[ignore]` until implementation ready
5. **Add to README** - Document what the test verifies

## CI/CD Integration

These tests run in CI on:
- Every pull request
- Main branch commits
- Nightly builds

CI configuration: `.github/workflows/integration-tests.yml` (to be created)

## Questions?

See the main project documentation:
- Architecture: `../architecture/ARCHITECTURE.md`
- Specification: `../specs/SPECIFICATION.md`
- Pseudocode: `../architecture/PSEUDOCODE.md`
