# Integration Test Implementation Guide

This guide helps implementers understand and use the integration tests for the EXO-AI 2025 cognitive substrate.

## Philosophy: Test-Driven Development

The integration tests in this project are written **BEFORE** implementation. This provides several benefits:

1. **Clear API Specifications** - Tests show exactly what interfaces are expected
2. **Executable Documentation** - Tests demonstrate how to use the system
3. **Implementation Guidance** - Tests guide implementation priorities
4. **Quality Assurance** - Passing tests verify correctness

## Quick Start for Implementers

### Step 1: Choose a Component

Start with one of these components:

- **exo-core** (foundational traits) - Start here
- **exo-backend-classical** (ruvector integration) - Depends on exo-core
- **exo-manifold** (learned storage) - Depends on exo-core
- **exo-hypergraph** (topology) - Depends on exo-core
- **exo-temporal** (causal memory) - Depends on exo-core
- **exo-federation** (distributed) - Depends on all above

### Step 2: Read the Tests

Find the relevant test file:

```bash
cd tests/
ls -la
# substrate_integration.rs - For exo-core/backend
# hypergraph_integration.rs - For exo-hypergraph
# temporal_integration.rs - For exo-temporal
# federation_integration.rs - For exo-federation
```

Read the test to understand expected behavior:

```rust
#[tokio::test]
#[ignore]
async fn test_substrate_store_and_retrieve() {
    // This shows the expected API:
    let config = SubstrateConfig::default();
    let backend = ClassicalBackend::new(config).unwrap();
    let substrate = SubstrateInstance::new(backend);

    // ... rest of test shows expected behavior
}
```

### Step 3: Implement to Pass Tests

Create the crate structure:

```bash
cd crates/
mkdir exo-core
cd exo-core
cargo init --lib
```

Implement the types and methods shown in the test:

```rust
// crates/exo-core/src/lib.rs
pub struct SubstrateConfig {
    // fields based on test usage
}

pub struct SubstrateInstance {
    // implementation
}

impl SubstrateInstance {
    pub fn new(backend: impl SubstrateBackend) -> Self {
        // implementation
    }

    pub async fn store(&self, pattern: Pattern) -> Result<PatternId, Error> {
        // implementation
    }

    pub async fn search(&self, query: Query, k: usize) -> Result<Vec<SearchResult>, Error> {
        // implementation
    }
}
```

### Step 4: Remove #[ignore] and Test

```rust
// Remove this line:
// #[ignore]

#[tokio::test]
async fn test_substrate_store_and_retrieve() {
    // test code...
}
```

Run the test:

```bash
cargo test --test substrate_integration test_substrate_store_and_retrieve
```

### Step 5: Iterate Until Passing

Fix compilation errors, then runtime errors, until:

```
test substrate_tests::test_substrate_store_and_retrieve ... ok
```

## Detailed Component Guides

### Implementing exo-core

**Priority Order:**

1. **Core Types** - Pattern, Query, Metadata, SubstrateTime
2. **Backend Trait** - SubstrateBackend trait definition
3. **Substrate Instance** - Main API facade
4. **Error Types** - Comprehensive error handling

**Key Tests:**

```bash
cargo test --test substrate_integration test_substrate_store_and_retrieve
cargo test --test substrate_integration test_filtered_search
cargo test --test substrate_integration test_bulk_operations
```

**Expected API Surface:**

```rust
// Types
pub struct Pattern {
    pub embedding: Vec<f32>,
    pub metadata: Metadata,
    pub timestamp: SubstrateTime,
    pub antecedents: Vec<PatternId>,
}

pub struct Query {
    embedding: Vec<f32>,
    filter: Option<Filter>,
}

pub struct SearchResult {
    pub id: PatternId,
    pub pattern: Pattern,
    pub score: f32,
}

// Traits
pub trait SubstrateBackend: Send + Sync {
    type Error: std::error::Error;

    fn similarity_search(
        &self,
        query: &[f32],
        k: usize,
        filter: Option<&Filter>,
    ) -> Result<Vec<SearchResult>, Self::Error>;

    // ... other methods
}

// Main API
pub struct SubstrateInstance {
    backend: Arc<dyn SubstrateBackend>,
}

impl SubstrateInstance {
    pub fn new(backend: impl SubstrateBackend + 'static) -> Self;
    pub async fn store(&self, pattern: Pattern) -> Result<PatternId, Error>;
    pub async fn search(&self, query: Query, k: usize) -> Result<Vec<SearchResult>, Error>;
}
```

### Implementing exo-manifold

**Depends On:** exo-core, burn framework

**Priority Order:**

1. **Manifold Network** - Neural network architecture (SIREN layers)
2. **Gradient Descent Retrieval** - Query via optimization
3. **Continuous Deformation** - Learning without discrete insert
4. **Forgetting Mechanism** - Strategic memory decay

**Key Tests:**

```bash
cargo test --test substrate_integration test_manifold_deformation
cargo test --test substrate_integration test_strategic_forgetting
```

**Expected Architecture:**

```rust
use burn::prelude::*;

pub struct ManifoldEngine<B: Backend> {
    network: LearnedManifold<B>,
    optimizer: AdamOptimizer<B>,
    config: ManifoldConfig,
}

impl<B: Backend> ManifoldEngine<B> {
    pub fn retrieve(&self, query: Tensor<B, 1>, k: usize) -> Vec<(Pattern, f32)> {
        // Gradient descent on manifold
    }

    pub fn deform(&mut self, pattern: Pattern, salience: f32) {
        // Continuous learning
    }

    pub fn forget(&mut self, region: &ManifoldRegion, decay_rate: f32) {
        // Strategic forgetting
    }
}
```

### Implementing exo-hypergraph

**Depends On:** exo-core, petgraph, ruvector-graph

**Priority Order:**

1. **Hyperedge Storage** - Multi-entity relationships
2. **Topological Queries** - Basic graph queries
3. **Persistent Homology** - TDA integration (teia crate)
4. **Sheaf Structures** - Advanced consistency (optional)

**Key Tests:**

```bash
cargo test --test hypergraph_integration test_hyperedge_creation_and_query
cargo test --test hypergraph_integration test_persistent_homology
cargo test --test hypergraph_integration test_betti_numbers
```

**Expected Architecture:**

```rust
use ruvector_graph::GraphDatabase;
use petgraph::Graph;

pub struct HypergraphSubstrate {
    base: GraphDatabase,
    hyperedges: HyperedgeIndex,
    topology: SimplicialComplex,
    sheaf: Option<SheafStructure>,
}

impl HypergraphSubstrate {
    pub async fn create_hyperedge(
        &mut self,
        entities: &[EntityId],
        relation: &Relation,
    ) -> Result<HyperedgeId, Error>;

    pub async fn persistent_homology(
        &self,
        dimension: usize,
        epsilon_range: (f32, f32),
    ) -> Result<PersistenceDiagram, Error>;

    pub async fn betti_numbers(&self, max_dim: usize) -> Result<Vec<usize>, Error>;
}
```

### Implementing exo-temporal

**Depends On:** exo-core

**Priority Order:**

1. **Causal Graph** - Antecedent tracking
2. **Causal Queries** - Cone-based retrieval
3. **Memory Consolidation** - Short-term to long-term
4. **Predictive Pre-fetch** - Anticipation

**Key Tests:**

```bash
cargo test --test temporal_integration test_causal_storage_and_query
cargo test --test temporal_integration test_memory_consolidation
cargo test --test temporal_integration test_predictive_anticipation
```

**Expected Architecture:**

```rust
pub struct TemporalMemory {
    short_term: ShortTermBuffer,
    long_term: LongTermStore,
    causal_graph: CausalGraph,
    tkg: TemporalKnowledgeGraph,
}

impl TemporalMemory {
    pub async fn store(
        &mut self,
        pattern: Pattern,
        antecedents: &[PatternId],
    ) -> Result<PatternId, Error>;

    pub async fn causal_query(
        &self,
        query: &Query,
        reference_time: SubstrateTime,
        cone_type: CausalConeType,
    ) -> Result<Vec<CausalResult>, Error>;

    pub async fn consolidate(&mut self) -> Result<(), Error>;

    pub async fn anticipate(&mut self, hints: &[AnticipationHint]) -> Result<(), Error>;
}
```

### Implementing exo-federation

**Depends On:** exo-core, exo-temporal, ruvector-raft, kyberlib

**Priority Order:**

1. **CRDT Merge** - Conflict-free reconciliation
2. **Post-Quantum Handshake** - Kyber key exchange
3. **Byzantine Consensus** - PBFT-style agreement
4. **Onion Routing** - Privacy-preserving queries

**Key Tests:**

```bash
cargo test --test federation_integration test_crdt_merge_reconciliation
cargo test --test federation_integration test_byzantine_consensus
cargo test --test federation_integration test_post_quantum_handshake
```

**Expected Architecture:**

```rust
use ruvector_raft::RaftNode;
use kyberlib::{encapsulate, decapsulate};

pub struct FederatedMesh {
    local: Arc<SubstrateInstance>,
    consensus: RaftNode,
    gateway: FederationGateway,
    pq_keys: PostQuantumKeypair,
}

impl FederatedMesh {
    pub async fn join_federation(
        &mut self,
        peer: &PeerAddress,
    ) -> Result<FederationToken, Error>;

    pub async fn federated_query(
        &self,
        query: &Query,
        scope: FederationScope,
    ) -> Result<Vec<FederatedResult>, Error>;

    pub async fn byzantine_commit(
        &self,
        update: &StateUpdate,
    ) -> Result<CommitProof, Error>;

    pub async fn merge_crdt_state(&mut self, state: CrdtState) -> Result<(), Error>;
}
```

## Common Implementation Patterns

### Async-First Design

All integration tests use `tokio::test`. Implement async throughout:

```rust
#[tokio::test]
async fn test_example() {
    let result = substrate.async_operation().await.unwrap();
}
```

### Error Handling

Use `Result<T, Error>` everywhere. Tests call `.unwrap()` or `.expect()`:

```rust
pub async fn store(&self, pattern: Pattern) -> Result<PatternId, Error> {
    // Implementation
}

// In tests:
let id = substrate.store(pattern).await.unwrap();
```

### Test Utilities

Leverage the test helpers:

```rust
use common::fixtures::*;
use common::assertions::*;
use common::helpers::*;

#[tokio::test]
async fn test_example() {
    init_test_logger();

    let embeddings = generate_test_embeddings(100, 128);
    let results = substrate.search(query, 10).await.unwrap();

    assert_scores_descending(&results.iter().map(|r| r.score).collect::<Vec<_>>());
}
```

## Debugging Integration Test Failures

### Enable Logging

```bash
RUST_LOG=debug cargo test --test substrate_integration -- --nocapture
```

### Run Single Test

```bash
cargo test --test substrate_integration test_substrate_store_and_retrieve -- --exact --nocapture
```

### Add Debug Prints

```rust
#[tokio::test]
async fn test_example() {
    let result = substrate.search(query, 10).await.unwrap();
    dbg!(&result); // Debug print
    assert_eq!(result.len(), 10);
}
```

### Use Breakpoints

With VS Code + rust-analyzer:

1. Set breakpoint in test or implementation
2. Run "Debug Test" from code lens
3. Step through execution

## Performance Profiling

### Measure Test Duration

```rust
use common::helpers::measure_async;

#[tokio::test]
async fn test_performance() {
    let (result, duration) = measure_async(async {
        substrate.search(query, 10).await.unwrap()
    }).await;

    assert!(duration.as_millis() < 10, "Query too slow: {:?}", duration);
}
```

### Benchmark Mode

```bash
cargo test --test substrate_integration --release -- --nocapture
```

## Coverage Analysis

Generate coverage reports:

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --workspace --out Html --output-dir coverage
open coverage/index.html
```

Target: >80% coverage for implemented crates.

## CI/CD Integration

Tests run automatically on:

- Pull requests (all tests)
- Main branch (all tests + coverage)
- Nightly (all tests + benchmarks)

See: `.github/workflows/integration-tests.yml`

## FAQ

### Q: All tests are ignored. How do I start?

**A:** Pick a test, implement the required types/methods, remove `#[ignore]`, run the test.

### Q: Test expects types I haven't implemented yet?

**A:** Implement them! The test shows exactly what's needed.

### Q: Can I modify the tests?

**A:** Generally no - tests define the contract. If a test is wrong, discuss with the team first.

### Q: How do I add new integration tests?

**A:** Follow existing patterns, add to relevant file, document in tests/README.md.

### Q: Tests depend on each other?

**A:** They shouldn't. Each test should be independent. Use test fixtures for shared setup.

### Q: How do I mock dependencies?

**A:** Use the fixtures in `common/fixtures.rs` or create test-specific mocks.

## Getting Help

- **Architecture Questions**: See `../architecture/ARCHITECTURE.md`
- **API Questions**: Read the test code - it shows expected usage
- **Implementation Questions**: Check pseudocode in `../architecture/PSEUDOCODE.md`
- **General Questions**: Open a GitHub issue

## Success Checklist

Before marking a component "done":

- [ ] All relevant integration tests pass (not ignored)
- [ ] Code coverage > 80%
- [ ] No compiler warnings
- [ ] Documentation written (rustdoc)
- [ ] Examples added to crate
- [ ] Performance targets met (see tests/README.md)
- [ ] Code reviewed by team

## Next Steps

1. Read the architecture: `../architecture/ARCHITECTURE.md`
2. Pick a component (recommend starting with exo-core)
3. Read its integration tests
4. Implement to pass tests
5. Submit PR with passing tests

Good luck! The tests are your guide. Trust the TDD process.
