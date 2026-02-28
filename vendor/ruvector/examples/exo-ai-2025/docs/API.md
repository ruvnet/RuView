# EXO-AI 2025 Cognitive Substrate - API Documentation

> **Version**: 0.1.0
> **License**: MIT OR Apache-2.0
> **Repository**: https://github.com/ruvnet/ruvector

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Core Crates](#core-crates)
4. [API Reference](#api-reference)
5. [Type System](#type-system)
6. [Error Handling](#error-handling)
7. [Migration from RuVector](#migration-from-ruvector)

---

## Overview

EXO-AI 2025 is a next-generation **cognitive substrate** designed for advanced AI systems. Unlike traditional vector databases that use discrete storage, EXO implements:

- **Continuous Manifold Storage** via implicit neural representations (SIREN networks)
- **Higher-Order Reasoning** through hypergraphs with topological data analysis
- **Temporal Causality** with short-term/long-term memory coordination
- **Distributed Cognition** using post-quantum federated mesh networking

### Key Features

| Feature | Description |
|---------|-------------|
| **Manifold Engine** | No discrete inserts—continuous deformation of learned space |
| **Hypergraph Substrate** | Relations spanning >2 entities, persistent homology, Betti numbers |
| **Temporal Memory** | Causal tracking, consolidation, anticipatory pre-fetching |
| **Federation** | Post-quantum crypto, onion routing, CRDT reconciliation, Byzantine consensus |
| **Multi-Platform** | Native Rust, WASM (browser), Node.js bindings |

---

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                    EXO-AI 2025 Stack                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  exo-wasm    │  │  exo-node    │  │  exo-cli     │      │
│  │  (Browser)   │  │  (Node.js)   │  │  (Native)    │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         └─────────────────┴─────────────────┘              │
│                           │                                 │
│  ┌────────────────────────┴────────────────────────┐        │
│  │              exo-core (Core Traits)             │        │
│  │  • SubstrateBackend                             │        │
│  │  • TemporalContext                              │        │
│  │  • Pattern, Query, SearchResult                 │        │
│  └────────────────────────────────────────────────┘        │
│         │              │              │              │      │
│  ┌──────▼──────┐┌─────▼─────┐┌──────▼──────┐┌─────▼─────┐ │
│  │ exo-manifold││exo-hyper-  ││exo-temporal ││exo-feder- │ │
│  │             ││  graph     ││             ││  ation    │ │
│  │ SIREN nets  ││ TDA/sheaf  ││ Causal mem  ││ P2P mesh  │ │
│  └─────────────┘└────────────┘└─────────────┘└───────────┘ │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Core Crates

### 1. **exo-core** - Foundation

Core trait definitions and types for the cognitive substrate.

**Key Exports:**
- `Pattern` - Vector embedding with metadata, causal antecedents, and salience
- `SubstrateBackend` - Hardware-agnostic backend trait
- `TemporalContext` - Temporal memory operations trait
- `Error` / `Result` - Unified error handling

**Example:**
```rust
use exo_core::{Pattern, PatternId, Metadata, SubstrateTime};

let pattern = Pattern {
    id: PatternId::new(),
    embedding: vec![1.0, 2.0, 3.0],
    metadata: Metadata::default(),
    timestamp: SubstrateTime::now(),
    antecedents: vec![],
    salience: 0.95,
};
```

---

### 2. **exo-manifold** - Learned Continuous Storage

Implements continuous manifold storage using **SIREN networks** (implicit neural representations).

**Key Exports:**
- `ManifoldEngine<B: Backend>` - Main engine for manifold operations
- `LearnedManifold<B>` - SIREN network implementation
- `GradientDescentRetriever` - Query via gradient descent
- `ManifoldDeformer` - Continuous deformation (replaces insert)
- `StrategicForgetting` - Manifold smoothing for low-salience regions

**Core Concept:**
Instead of discrete vector insertion, patterns **deform** the learned manifold:

```rust
use exo_manifold::{ManifoldEngine, ManifoldConfig};
use burn::backend::NdArray;

let config = ManifoldConfig {
    dimension: 768,
    max_descent_steps: 100,
    learning_rate: 0.01,
    convergence_threshold: 1e-4,
    hidden_layers: 3,
    hidden_dim: 256,
    omega_0: 30.0,
};

let device = Default::default();
let mut engine = ManifoldEngine::<NdArray>::new(config, device);

// Continuous deformation (no discrete insert)
let delta = engine.deform(pattern, salience)?;

// Retrieval via gradient descent
let results = engine.retrieve(&query_embedding, k)?;

// Strategic forgetting
let pruned_count = engine.forget(0.1, 0.95)?;
```

---

### 3. **exo-hypergraph** - Higher-Order Relations

Supports **hyperedges** (relations spanning >2 entities) with topological data analysis.

**Key Exports:**
- `HypergraphSubstrate` - Main hypergraph structure
- `Hyperedge` - N-way relation
- `SimplicialComplex` - For persistent homology
- `SheafStructure` - Consistency checking

**Topological Queries:**
- **Persistent Homology** - Find topological features across scales
- **Betti Numbers** - Count connected components, loops, voids
- **Sheaf Consistency** - Local-to-global coherence checks

**Example:**
```rust
use exo_hypergraph::{HypergraphSubstrate, HypergraphConfig};
use exo_core::{EntityId, Relation, RelationType};

let config = HypergraphConfig {
    enable_sheaf: true,
    max_dimension: 3,
    epsilon: 1e-6,
};

let mut hypergraph = HypergraphSubstrate::new(config);

// Create 3-way hyperedge
let entities = [EntityId::new(), EntityId::new(), EntityId::new()];
for &e in &entities {
    hypergraph.add_entity(e, serde_json::json!({}));
}

let relation = Relation {
    relation_type: RelationType::new("collaboration"),
    properties: serde_json::json!({"weight": 0.9}),
};

let hyperedge_id = hypergraph.create_hyperedge(&entities, &relation)?;

// Topological queries
let betti = hypergraph.betti_numbers(3); // [β₀, β₁, β₂, β₃]
let diagram = hypergraph.persistent_homology(1, (0.0, 1.0));
```

---

### 4. **exo-temporal** - Temporal Memory

Implements temporal memory with **causal tracking** and **consolidation**.

**Key Exports:**
- `TemporalMemory` - Main coordinator
- `ShortTermBuffer` - Volatile recent memory
- `LongTermStore` - Consolidated persistent memory
- `CausalGraph` - DAG of causal relationships
- `AnticipationHint` / `PrefetchCache` - Predictive retrieval

**Memory Layers:**
1. **Short-Term**: Volatile buffer (recent patterns)
2. **Long-Term**: Consolidated store (high-salience patterns)
3. **Causal Graph**: Tracks antecedent relationships

**Example:**
```rust
use exo_temporal::{TemporalMemory, TemporalConfig, CausalConeType};

let memory = TemporalMemory::new(TemporalConfig::default());

// Store with causal context
let p1 = Pattern::new(vec![1.0, 0.0, 0.0], Metadata::new());
let id1 = memory.store(p1, &[])?;

let p2 = Pattern::new(vec![0.9, 0.1, 0.0], Metadata::new());
let id2 = memory.store(p2, &[id1])?; // p2 caused by p1

// Causal query (within past light-cone)
let query = Query::from_embedding(vec![1.0, 0.0, 0.0]).with_origin(id1);
let results = memory.causal_query(
    &query,
    SubstrateTime::now(),
    CausalConeType::Past,
);

// Consolidation: short-term → long-term
let consolidation_result = memory.consolidate();
```

---

### 5. **exo-federation** - Distributed Mesh

Federated substrate networking with **post-quantum cryptography** and **Byzantine consensus**.

**Key Exports:**
- `FederatedMesh` - Main coordinator
- `PostQuantumKeypair` - Dilithium/Kyber keys
- `join_federation()` - Handshake protocol
- `onion_query()` - Privacy-preserving routing
- `byzantine_commit()` - BFT consensus (f = ⌊(N-1)/3⌋)

**Features:**
- **Post-Quantum Crypto**: CRYSTALS-Dilithium + Kyber
- **Onion Routing**: Multi-hop privacy (Tor-like)
- **CRDT Reconciliation**: Eventual consistency
- **Byzantine Consensus**: 3f+1 fault tolerance

**Example:**
```rust
use exo_federation::{FederatedMesh, PeerAddress, FederationScope};

let local_substrate = SubstrateInstance::new(config)?;
let mut mesh = FederatedMesh::new(local_substrate)?;

// Join federation
let peer = PeerAddress::new(
    "peer.example.com".to_string(),
    9000,
    peer_public_key,
);
let token = mesh.join_federation(&peer).await?;

// Federated query
let results = mesh.federated_query(
    query_data,
    FederationScope::Global { max_hops: 3 },
).await?;

// Byzantine consensus for state update
let update = StateUpdate { /* ... */ };
let proof = mesh.byzantine_commit(update).await?;
```

---

### 6. **exo-wasm** - Browser Bindings

WASM bindings for browser-based cognitive substrate.

**Key Exports:**
- `ExoSubstrate` - Main WASM interface
- `Pattern` - WASM-compatible pattern type
- `SearchResult` - WASM search result

**Example (JavaScript):**
```javascript
import init, { ExoSubstrate } from 'exo-wasm';

await init();

const substrate = new ExoSubstrate({
  dimensions: 384,
  distance_metric: "cosine",
  use_hnsw: true,
  enable_temporal: true,
  enable_causal: true
});

// Store pattern
const pattern = new Pattern(
  new Float32Array([1.0, 2.0, 3.0, ...]),
  { text: "example", category: "demo" },
  [] // antecedents
);
const id = substrate.store(pattern);

// Query
const results = await substrate.query(
  new Float32Array([1.0, 2.0, 3.0, ...]),
  10
);

// Stats
const stats = substrate.stats();
console.log(`Patterns: ${stats.pattern_count}`);
```

---

### 7. **exo-node** - Node.js Bindings

High-performance Node.js bindings via **NAPI-RS**.

**Key Exports:**
- `ExoSubstrateNode` - Main Node.js interface
- `version()` - Get library version

**Example (Node.js/TypeScript):**
```typescript
import { ExoSubstrateNode } from 'exo-node';

const substrate = new ExoSubstrateNode({
  dimensions: 384,
  storagePath: './substrate.db',
  enableHypergraph: true,
  enableTemporal: true
});

// Store pattern
const id = await substrate.store({
  embedding: new Float32Array([1.0, 2.0, 3.0]),
  metadata: { text: 'example' },
  antecedents: []
});

// Search
const results = await substrate.search(
  new Float32Array([1.0, 2.0, 3.0]),
  10
);

// Hypergraph query
const hypergraphResult = await substrate.hypergraphQuery(
  JSON.stringify({
    type: 'BettiNumbers',
    maxDimension: 3
  })
);

// Stats
const stats = await substrate.stats();
```

---

## Type System

### Core Types

#### `Pattern`
Vector embedding with causal and temporal context.

```rust
pub struct Pattern {
    pub id: PatternId,
    pub embedding: Vec<f32>,
    pub metadata: Metadata,
    pub timestamp: SubstrateTime,
    pub antecedents: Vec<PatternId>,  // Causal dependencies
    pub salience: f32,                 // Importance score [0, 1]
}
```

#### `PatternId`
Unique identifier for patterns (UUID).

```rust
pub struct PatternId(pub Uuid);

impl PatternId {
    pub fn new() -> Self;
}
```

#### `SubstrateTime`
Nanosecond-precision timestamp.

```rust
pub struct SubstrateTime(pub i64);

impl SubstrateTime {
    pub const MIN: Self;
    pub const MAX: Self;
    pub fn now() -> Self;
    pub fn abs(&self) -> Self;
}
```

#### `SearchResult`
Result from similarity search.

```rust
pub struct SearchResult {
    pub pattern: Pattern,
    pub score: f32,      // Similarity score
    pub distance: f32,   // Distance metric
}
```

#### `Filter`
Metadata filtering for queries.

```rust
pub struct Filter {
    pub conditions: Vec<FilterCondition>,
}

pub struct FilterCondition {
    pub field: String,
    pub operator: FilterOperator,  // Equal, NotEqual, GreaterThan, LessThan, Contains
    pub value: MetadataValue,
}
```

---

### Hypergraph Types

#### `Hyperedge`
N-way relation spanning multiple entities.

```rust
pub struct Hyperedge {
    pub id: HyperedgeId,
    pub entities: Vec<EntityId>,
    pub relation: Relation,
}
```

#### `TopologicalQuery`
Query specification for TDA operations.

```rust
pub enum TopologicalQuery {
    PersistentHomology {
        dimension: usize,
        epsilon_range: (f32, f32),
    },
    BettiNumbers {
        max_dimension: usize,
    },
    SheafConsistency {
        local_sections: Vec<SectionId>,
    },
}
```

#### `HyperedgeResult`
Result from topological queries.

```rust
pub enum HyperedgeResult {
    PersistenceDiagram(Vec<(f32, f32)>),  // (birth, death) pairs
    BettiNumbers(Vec<usize>),              // [β₀, β₁, β₂, ...]
    SheafConsistency(SheafConsistencyResult),
}
```

---

### Temporal Types

#### `CausalResult`
Search result with causal and temporal context.

```rust
pub struct CausalResult {
    pub pattern: Pattern,
    pub similarity: f32,
    pub causal_distance: Option<usize>,  // Hops in causal graph
    pub temporal_distance: Duration,
    pub combined_score: f32,
}
```

#### `CausalConeType`
Causal cone constraint for queries.

```rust
pub enum CausalConeType {
    Past,                    // Only past events
    Future,                  // Only future events
    LightCone { radius: f32 }, // Relativistic constraint
}
```

#### `AnticipationHint`
Hint for predictive pre-fetching.

```rust
pub enum AnticipationHint {
    Sequential {
        last_k_patterns: Vec<PatternId>,
    },
    Temporal {
        current_phase: TemporalPhase,
    },
    Contextual {
        active_context: Vec<PatternId>,
    },
}
```

---

### Federation Types

#### `PeerId`
Unique identifier for federation peers.

```rust
pub struct PeerId(pub String);

impl PeerId {
    pub fn generate() -> Self;
}
```

#### `FederationScope`
Scope for federated queries.

```rust
pub enum FederationScope {
    Local,                    // Query only local instance
    Direct,                   // Query direct peers
    Global { max_hops: usize }, // Multi-hop query
}
```

#### `FederatedResult`
Result from federated query.

```rust
pub struct FederatedResult {
    pub source: PeerId,
    pub data: Vec<u8>,
    pub score: f32,
    pub timestamp: u64,
}
```

---

## Error Handling

All crates use a unified error model with `thiserror`.

### `exo_core::Error`

```rust
pub enum Error {
    PatternNotFound(PatternId),
    InvalidDimension { expected: usize, got: usize },
    Backend(String),
    ConvergenceFailed,
    InvalidConfig(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

### `exo_temporal::TemporalError`

```rust
pub enum TemporalError {
    PatternNotFound(PatternId),
    InvalidQuery(String),
    StorageError(String),
}
```

### `exo_federation::FederationError`

```rust
pub enum FederationError {
    CryptoError(String),
    NetworkError(String),
    ConsensusError(String),
    InvalidToken,
    InsufficientPeers { needed: usize, actual: usize },
    ReconciliationError(String),
    PeerNotFound(String),
}
```

---

## Migration from RuVector

EXO-AI 2025 is the next evolution of RuVector. Here's how to migrate:

### Key Differences

| RuVector | EXO-AI 2025 |
|----------|-------------|
| **Discrete inserts** | **Continuous deformation** |
| `db.insert(vector)` | `engine.deform(pattern, salience)` |
| Simple vector DB | Cognitive substrate |
| No causal tracking | Full causal graph |
| No hypergraph support | Full TDA + sheaf theory |
| Single-node only | Distributed federation |

### Migration Example

**Before (RuVector):**
```rust
use ruvector_core::{VectorDB, VectorEntry};

let db = VectorDB::new(db_options)?;

let entry = VectorEntry {
    id: Some("doc1".to_string()),
    vector: vec![1.0, 2.0, 3.0],
    metadata: Some(metadata),
};

let id = db.insert(entry)?;
let results = db.search(search_query)?;
```

**After (EXO-AI 2025):**
```rust
use exo_manifold::{ManifoldEngine, ManifoldConfig};
use exo_core::Pattern;
use burn::backend::NdArray;

let config = ManifoldConfig::default();
let mut engine = ManifoldEngine::<NdArray>::new(config, device);

let pattern = Pattern {
    id: PatternId::new(),
    embedding: vec![1.0, 2.0, 3.0],
    metadata: Metadata::default(),
    timestamp: SubstrateTime::now(),
    antecedents: vec![],
    salience: 0.9,
};

// Continuous deformation instead of discrete insert
let delta = engine.deform(pattern, 0.9)?;

// Gradient descent retrieval
let results = engine.retrieve(&query, k)?;
```

### Backend Compatibility

For **classical discrete backends** (backward compatibility):

```rust
use exo_backend_classical::ClassicalBackend;
use exo_core::SubstrateBackend;

let backend = ClassicalBackend::new(config);

// Still uses discrete storage internally
backend.similarity_search(&query, k, filter)?;

// Deform becomes insert for classical backends
backend.manifold_deform(&pattern, learning_rate)?;
```

---

## Performance Characteristics

### Manifold Engine

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| `deform()` | O(H·D) | H=hidden layers, D=dimension |
| `retrieve()` | O(S·H·D) | S=descent steps |
| `forget()` | O(P·D) | P=patterns to prune |

### Hypergraph

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| `create_hyperedge()` | O(E) | E=entity count |
| `persistent_homology()` | O(N³) | N=simplex count |
| `betti_numbers()` | O(N²·d) | d=max dimension |

### Temporal Memory

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| `store()` | O(1) | Short-term insert |
| `causal_query()` | O(log N + k) | N=total patterns |
| `consolidate()` | O(S·log L) | S=short-term, L=long-term |

---

## Thread Safety

All crates are **thread-safe** by design:

- `ManifoldEngine`: Uses `Arc<RwLock<...>>`
- `HypergraphSubstrate`: Uses `DashMap` (lock-free)
- `TemporalMemory`: Uses `Arc` + concurrent data structures
- `FederatedMesh`: Async-safe with `tokio::sync::RwLock`

---

## Feature Flags

```toml
[features]
default = ["simd"]
simd = []           # SIMD optimizations
distributed = []    # Enable federation
gpu = []            # GPU backend support (future)
quantization = []   # Vector quantization (future)
```

---

## Version History

- **v0.1.0** (2025-01-29): Initial release
  - Manifold engine with SIREN networks
  - Hypergraph substrate with TDA
  - Temporal memory coordinator
  - Federation with post-quantum crypto
  - WASM and Node.js bindings

---

## See Also

- [Examples](./EXAMPLES.md) - Practical usage examples
- [Test Strategy](./TEST_STRATEGY.md) - Testing approach
- [Integration Guide](./INTEGRATION_TEST_GUIDE.md) - Integration testing
- [Performance Baseline](./PERFORMANCE_BASELINE.md) - Benchmarks

---

**Questions?** Open an issue at https://github.com/ruvnet/ruvector/issues
