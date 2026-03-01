# EXO-AI 2025: System Architecture

## SPARC Phase 3: Architecture Design

### Executive Summary

This document defines the modular architecture for an experimental cognitive substrate platform, consuming the ruvector ecosystem as an SDK while exploring technologies projected for 2035-2060.

---

## 1. Architectural Principles

### 1.1 Core Design Tenets

| Principle | Description | Implementation |
|-----------|-------------|----------------|
| **SDK Consumer** | No modifications to ruvector crates | Clean dependency boundaries |
| **Backend Agnostic** | Hardware abstraction via traits | PIM, neuromorphic, photonic backends |
| **Substrate-First** | Data and compute unified | In-memory operations where possible |
| **Topology Native** | Hypergraph as primary structure | Edges span arbitrary entity sets |
| **Temporal Coherent** | Causal memory by default | Every operation timestamped |

### 1.2 Layer Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    APPLICATION LAYER                             │
│  ┌─────────────┐ ┌──────────────┐ ┌───────────────────────────┐ │
│  │ Agent SDK   │ │ Query Engine │ │ Federation Gateway        │ │
│  └─────────────┘ └──────────────┘ └───────────────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│                    SUBSTRATE LAYER                               │
│  ┌─────────────┐ ┌──────────────┐ ┌───────────────────────────┐ │
│  │ Manifold    │ │ Hypergraph   │ │ Temporal Memory           │ │
│  │ Engine      │ │ Substrate    │ │ Coordinator               │ │
│  └─────────────┘ └──────────────┘ └───────────────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│                    BACKEND ABSTRACTION                           │
│  ┌─────────────┐ ┌──────────────┐ ┌───────────────────────────┐ │
│  │ Classical   │ │ Neuromorphic │ │ Photonic                  │ │
│  │ (ruvector)  │ │ (Future)     │ │ (Future)                  │ │
│  └─────────────┘ └──────────────┘ └───────────────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│                    INFRASTRUCTURE                                │
│  ┌─────────────┐ ┌──────────────┐ ┌───────────────────────────┐ │
│  │ WASM        │ │ NAPI-RS      │ │ Native                    │ │
│  │ Runtime     │ │ Bindings     │ │ Binaries                  │ │
│  └─────────────┘ └──────────────┘ └───────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. Module Design

### 2.1 Core Modules

```
exo-ai-2025/
├── crates/
│   ├── exo-core/              # Core traits and types
│   ├── exo-manifold/          # Learned manifold engine
│   ├── exo-hypergraph/        # Hypergraph substrate
│   ├── exo-temporal/          # Temporal memory coordinator
│   ├── exo-federation/        # Federated mesh networking
│   ├── exo-backend-classical/ # Classical backend (ruvector)
│   ├── exo-backend-sim/       # Neuromorphic/photonic simulator
│   ├── exo-wasm/              # WASM bindings
│   └── exo-node/              # NAPI-RS bindings
├── examples/
├── docs/
└── research/
```

### 2.2 exo-core: Foundational Traits

```rust
//! Core trait definitions for backend abstraction

/// Backend trait for substrate compute operations
pub trait SubstrateBackend: Send + Sync {
    type Error: std::error::Error;

    /// Execute similarity search on substrate
    fn similarity_search(
        &self,
        query: &[f32],
        k: usize,
        filter: Option<&Filter>,
    ) -> Result<Vec<SearchResult>, Self::Error>;

    /// Deform manifold to incorporate new pattern
    fn manifold_deform(
        &self,
        pattern: &Pattern,
        learning_rate: f32,
    ) -> Result<ManifoldDelta, Self::Error>;

    /// Execute hyperedge query
    fn hyperedge_query(
        &self,
        query: &TopologicalQuery,
    ) -> Result<HyperedgeResult, Self::Error>;
}

/// Temporal context for causal operations
pub trait TemporalContext {
    /// Get current substrate time
    fn now(&self) -> SubstrateTime;

    /// Query with causal cone constraints
    fn causal_query(
        &self,
        query: &Query,
        cone: &CausalCone,
    ) -> Result<Vec<CausalResult>, Error>;

    /// Predictive pre-fetch based on anticipated queries
    fn anticipate(&self, hints: &[AnticipationHint]) -> Result<(), Error>;
}

/// Pattern representation in substrate
#[derive(Clone, Debug)]
pub struct Pattern {
    /// Vector embedding
    pub embedding: Vec<f32>,
    /// Metadata
    pub metadata: Metadata,
    /// Temporal origin
    pub timestamp: SubstrateTime,
    /// Causal antecedents
    pub antecedents: Vec<PatternId>,
}

/// Topological query specification
#[derive(Clone, Debug)]
pub enum TopologicalQuery {
    /// Find persistent homology features
    PersistentHomology {
        dimension: usize,
        epsilon_range: (f32, f32),
    },
    /// Find N-dimensional holes in structure
    BettiNumbers {
        max_dimension: usize,
    },
    /// Sheaf consistency check
    SheafConsistency {
        local_sections: Vec<SectionId>,
    },
}
```

### 2.3 exo-manifold: Learned Representation Engine

```rust
//! Continuous manifold storage replacing discrete indices

use burn::prelude::*;
use crate::core::{Pattern, SubstrateBackend, ManifoldDelta};

/// Implicit Neural Representation for manifold storage
pub struct ManifoldEngine<B: Backend> {
    /// Neural network representing the manifold
    network: LearnedManifold<B>,
    /// Tensor Train decomposition for compression
    tt_decomposition: Option<TensorTrainConfig>,
    /// Consolidation scheduler
    consolidation: ConsolidationPolicy,
}

impl<B: Backend> ManifoldEngine<B> {
    /// Query manifold via gradient descent
    pub fn retrieve(
        &self,
        query: Tensor<B, 1>,
        k: usize,
    ) -> Vec<(Pattern, f32)> {
        // Initialize at query position
        let mut position = query.clone();

        // Gradient descent toward relevant memories
        for _ in 0..self.config.max_descent_steps {
            let relevance = self.network.forward(position.clone());
            let gradient = relevance.backward();
            position = position - self.config.learning_rate * gradient;

            if gradient.norm() < self.config.convergence_threshold {
                break;
            }
        }

        // Extract patterns from converged region
        self.extract_patterns_near(position, k)
    }

    /// Continuous manifold deformation (replaces insert)
    pub fn deform(&mut self, pattern: Pattern, salience: f32) {
        let embedding = Tensor::from_floats(&pattern.embedding);

        // Deformation = gradient update to manifold weights
        let loss = self.deformation_loss(embedding, salience);
        let gradients = loss.backward();

        self.optimizer.step(gradients);
    }

    /// Strategic forgetting via manifold smoothing
    pub fn forget(&mut self, region: &ManifoldRegion, decay_rate: f32) {
        // Smooth the manifold in low-salience regions
        self.apply_forgetting_kernel(region, decay_rate);
    }
}

/// Learned manifold network architecture
#[derive(Module)]
pub struct LearnedManifold<B: Backend> {
    /// SIREN-style sinusoidal layers
    layers: Vec<SirenLayer<B>>,
    /// Fourier feature encoding
    fourier_features: FourierEncoding<B>,
}
```

### 2.4 exo-hypergraph: Topological Substrate

```rust
//! Hypergraph substrate for higher-order relations

use petgraph::Graph;
use simplicial_topology::SimplicialComplex;
use ruvector_graph::{GraphDatabase, HyperedgeSupport};

/// Hypergraph substrate extending ruvector-graph
pub struct HypergraphSubstrate {
    /// Base graph from ruvector-graph
    base: GraphDatabase,
    /// Hyperedge index (relations spanning >2 entities)
    hyperedges: HyperedgeIndex,
    /// Simplicial complex for TDA
    topology: SimplicialComplex,
    /// Sheaf structure for consistency
    sheaf: Option<SheafStructure>,
}

impl HypergraphSubstrate {
    /// Create hyperedge spanning multiple entities
    pub fn create_hyperedge(
        &mut self,
        entities: &[EntityId],
        relation: &Relation,
    ) -> Result<HyperedgeId, Error> {
        // Validate entity existence
        for entity in entities {
            self.base.get_node(*entity)?;
        }

        // Create hyperedge in index
        let hyperedge_id = self.hyperedges.insert(entities, relation);

        // Update simplicial complex
        self.topology.add_simplex(entities);

        // Update sheaf sections if enabled
        if let Some(ref mut sheaf) = self.sheaf {
            sheaf.update_sections(hyperedge_id, entities)?;
        }

        Ok(hyperedge_id)
    }

    /// Topological query: find persistent features
    pub fn persistent_homology(
        &self,
        dimension: usize,
        epsilon_range: (f32, f32),
    ) -> PersistenceDiagram {
        use teia::persistence::compute_persistence;

        let filtration = self.topology.filtration(epsilon_range);
        compute_persistence(&filtration, dimension)
    }

    /// Query Betti numbers (topological invariants)
    pub fn betti_numbers(&self, max_dim: usize) -> Vec<usize> {
        (0..=max_dim)
            .map(|d| self.topology.betti_number(d))
            .collect()
    }

    /// Sheaf consistency: check local-to-global coherence
    pub fn check_sheaf_consistency(
        &self,
        sections: &[SectionId],
    ) -> SheafConsistencyResult {
        match &self.sheaf {
            Some(sheaf) => sheaf.check_consistency(sections),
            None => SheafConsistencyResult::NotConfigured,
        }
    }
}

/// Hyperedge index structure
struct HyperedgeIndex {
    /// Hyperedge storage
    edges: DashMap<HyperedgeId, Hyperedge>,
    /// Inverted index: entity -> hyperedges containing it
    entity_index: DashMap<EntityId, Vec<HyperedgeId>>,
    /// Relation type index
    relation_index: DashMap<RelationType, Vec<HyperedgeId>>,
}
```

### 2.5 exo-temporal: Causal Memory Coordinator

```rust
//! Temporal memory with causal structure

use std::collections::BTreeMap;
use ruvector_core::VectorIndex;

/// Temporal memory coordinator
pub struct TemporalMemory {
    /// Short-term volatile memory
    short_term: ShortTermBuffer,
    /// Long-term consolidated memory
    long_term: LongTermStore,
    /// Causal graph tracking antecedent relationships
    causal_graph: CausalGraph,
    /// Temporal knowledge graph (Zep-inspired)
    tkg: TemporalKnowledgeGraph,
}

impl TemporalMemory {
    /// Store with causal context
    pub fn store(
        &mut self,
        pattern: Pattern,
        antecedents: &[PatternId],
    ) -> Result<PatternId, Error> {
        // Add to short-term buffer
        let id = self.short_term.insert(pattern.clone());

        // Record causal relationships
        for antecedent in antecedents {
            self.causal_graph.add_edge(*antecedent, id);
        }

        // Update TKG with temporal relations
        self.tkg.add_temporal_fact(id, &pattern, antecedents)?;

        // Schedule consolidation if buffer full
        if self.short_term.should_consolidate() {
            self.trigger_consolidation();
        }

        Ok(id)
    }

    /// Causal cone query: retrieve within light-cone constraints
    pub fn causal_query(
        &self,
        query: &Query,
        reference_time: SubstrateTime,
        cone_type: CausalConeType,
    ) -> Vec<CausalResult> {
        // Determine valid time range based on cone
        let time_range = match cone_type {
            CausalConeType::Past => (SubstrateTime::MIN, reference_time),
            CausalConeType::Future => (reference_time, SubstrateTime::MAX),
            CausalConeType::LightCone { velocity } => {
                self.compute_light_cone(reference_time, velocity)
            }
        };

        // Query with temporal filter
        self.long_term
            .search_with_time_range(query, time_range)
            .into_iter()
            .map(|r| CausalResult {
                pattern: r.pattern,
                causal_distance: self.causal_graph.distance(r.id, query.origin),
                temporal_distance: (r.timestamp - reference_time).abs(),
            })
            .collect()
    }

    /// Anticipatory pre-fetch for predictive retrieval
    pub fn anticipate(&mut self, hints: &[AnticipationHint]) {
        for hint in hints {
            // Pre-compute likely future queries
            let predicted_queries = self.predict_future_queries(hint);

            // Warm cache with predicted results
            for query in predicted_queries {
                self.prefetch_cache.insert(query.hash(),
                    self.long_term.search(&query));
            }
        }
    }

    /// Memory consolidation: short-term -> long-term
    fn consolidate(&mut self) {
        // Identify salient patterns
        let salient = self.short_term
            .drain()
            .filter(|p| p.salience > self.consolidation_threshold);

        // Compress via manifold integration
        for pattern in salient {
            self.long_term.integrate(pattern);
        }

        // Strategic forgetting in long-term
        self.long_term.decay_low_salience(self.decay_rate);
    }
}

/// Causal graph for tracking antecedent relationships
struct CausalGraph {
    /// Forward edges: cause -> effects
    forward: DashMap<PatternId, Vec<PatternId>>,
    /// Backward edges: effect -> causes
    backward: DashMap<PatternId, Vec<PatternId>>,
}
```

### 2.6 exo-federation: Distributed Cognitive Mesh

```rust
//! Federated substrate with cryptographic sovereignty

use ruvector_raft::{RaftNode, RaftConfig};
use ruvector_cluster::ClusterManager;
use kyberlib::{keypair, encapsulate, decapsulate};

/// Federated cognitive mesh
pub struct FederatedMesh {
    /// Local substrate instance
    local: Arc<SubstrateInstance>,
    /// Raft consensus for local cluster
    consensus: RaftNode,
    /// Federation gateway
    gateway: FederationGateway,
    /// Post-quantum keypair
    pq_keys: PostQuantumKeypair,
}

impl FederatedMesh {
    /// Join federation with cryptographic handshake
    pub async fn join_federation(
        &mut self,
        peer: &PeerAddress,
    ) -> Result<FederationToken, Error> {
        // Post-quantum key exchange
        let (ciphertext, shared_secret) = encapsulate(&peer.public_key)?;

        // Establish encrypted channel
        let channel = self.gateway.establish_channel(
            peer,
            ciphertext,
            shared_secret,
        ).await?;

        // Exchange federation capabilities
        let token = channel.negotiate_federation().await?;

        Ok(token)
    }

    /// Federated query with privacy preservation
    pub async fn federated_query(
        &self,
        query: &Query,
        scope: FederationScope,
    ) -> Vec<FederatedResult> {
        // Route through onion network for intent privacy
        let onion_query = self.gateway.onion_wrap(query, scope)?;

        // Broadcast to federation peers
        let responses = self.gateway.broadcast(onion_query).await;

        // CRDT reconciliation for eventual consistency
        let reconciled = self.reconcile_crdt(responses)?;

        reconciled
    }

    /// Byzantine fault tolerant consensus on shared state
    pub async fn byzantine_commit(
        &self,
        update: &StateUpdate,
    ) -> Result<CommitProof, Error> {
        // Require 2f+1 agreement for n=3f+1 nodes
        let threshold = (self.peer_count() * 2 / 3) + 1;

        // Propose update
        let proposal = self.consensus.propose(update)?;

        // Collect votes
        let votes = self.gateway.collect_votes(proposal).await;

        if votes.len() >= threshold {
            Ok(CommitProof::from_votes(votes))
        } else {
            Err(Error::InsufficientConsensus)
        }
    }
}

/// Post-quantum cryptographic keypair
struct PostQuantumKeypair {
    /// CRYSTALS-Kyber public key
    public: [u8; 1184],
    /// CRYSTALS-Kyber secret key
    secret: [u8; 2400],
}
```

---

## 3. Backend Abstraction Layer

### 3.1 Classical Backend (ruvector SDK)

```rust
//! Classical backend consuming ruvector crates

use ruvector_core::{VectorIndex, HnswConfig};
use ruvector_graph::GraphDatabase;
use ruvector_gnn::GnnLayer;

/// Classical substrate backend using ruvector
pub struct ClassicalBackend {
    /// Vector index from ruvector-core
    vector_index: VectorIndex,
    /// Graph database from ruvector-graph
    graph_db: GraphDatabase,
    /// GNN layer from ruvector-gnn
    gnn: Option<GnnLayer>,
}

impl SubstrateBackend for ClassicalBackend {
    type Error = ruvector_core::Error;

    fn similarity_search(
        &self,
        query: &[f32],
        k: usize,
        filter: Option<&Filter>,
    ) -> Result<Vec<SearchResult>, Self::Error> {
        // Direct delegation to ruvector-core
        let results = match filter {
            Some(f) => self.vector_index.search_with_filter(query, k, f)?,
            None => self.vector_index.search(query, k)?,
        };

        Ok(results.into_iter().map(SearchResult::from).collect())
    }

    fn manifold_deform(
        &self,
        pattern: &Pattern,
        _learning_rate: f32,
    ) -> Result<ManifoldDelta, Self::Error> {
        // Classical backend: discrete insert
        let id = self.vector_index.insert(&pattern.embedding, &pattern.metadata)?;

        Ok(ManifoldDelta::DiscreteInsert { id })
    }

    fn hyperedge_query(
        &self,
        query: &TopologicalQuery,
    ) -> Result<HyperedgeResult, Self::Error> {
        // Use ruvector-graph hyperedge support
        match query {
            TopologicalQuery::PersistentHomology { .. } => {
                // Compute via graph traversal
                unimplemented!("TDA on classical backend")
            }
            TopologicalQuery::BettiNumbers { .. } => {
                // Approximate via connected components
                unimplemented!("Betti numbers on classical backend")
            }
            TopologicalQuery::SheafConsistency { .. } => {
                // Not supported on classical backend
                Ok(HyperedgeResult::NotSupported)
            }
        }
    }
}
```

### 3.2 Future Backend Traits

```rust
//! Placeholder traits for future hardware backends

/// Processing-in-Memory backend interface
pub trait PimBackend: SubstrateBackend {
    /// Execute operation in memory bank
    fn execute_in_memory(&self, op: &MemoryOperation) -> Result<(), Error>;

    /// Query memory bank location for data
    fn data_location(&self, pattern_id: PatternId) -> MemoryBank;
}

/// Neuromorphic backend interface
pub trait NeuromorphicBackend: SubstrateBackend {
    /// Encode vector as spike train
    fn encode_spikes(&self, vector: &[f32]) -> SpikeTrain;

    /// Decode spike train to vector
    fn decode_spikes(&self, spikes: &SpikeTrain) -> Vec<f32>;

    /// Submit spike computation
    fn submit_spike_compute(&self, input: SpikeTrain) -> Result<SpikeTrain, Error>;
}

/// Photonic backend interface
pub trait PhotonicBackend: SubstrateBackend {
    /// Optical matrix-vector multiply
    fn optical_matmul(&self, matrix: &OpticalMatrix, vector: &[f32]) -> Vec<f32>;

    /// Configure optical interference pattern
    fn configure_mzi(&self, config: &MziConfig) -> Result<(), Error>;
}
```

---

## 4. WASM & NAPI-RS Integration

### 4.1 WASM Module Structure

```rust
//! WASM bindings for browser/edge deployment

use wasm_bindgen::prelude::*;
use crate::core::{Pattern, Query};

#[wasm_bindgen]
pub struct ExoSubstrate {
    inner: Arc<SubstrateInstance>,
}

#[wasm_bindgen]
impl ExoSubstrate {
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<ExoSubstrate, JsError> {
        let config: SubstrateConfig = serde_wasm_bindgen::from_value(config)?;
        let inner = SubstrateInstance::new(config)?;
        Ok(Self { inner: Arc::new(inner) })
    }

    #[wasm_bindgen]
    pub async fn query(&self, embedding: Float32Array, k: u32) -> Result<JsValue, JsError> {
        let query = Query::from_embedding(embedding.to_vec());
        let results = self.inner.search(query, k as usize).await?;
        Ok(serde_wasm_bindgen::to_value(&results)?)
    }

    #[wasm_bindgen]
    pub fn store(&self, pattern: JsValue) -> Result<String, JsError> {
        let pattern: Pattern = serde_wasm_bindgen::from_value(pattern)?;
        let id = self.inner.store(pattern)?;
        Ok(id.to_string())
    }
}
```

### 4.2 NAPI-RS Bindings

```rust
//! Node.js bindings via NAPI-RS

use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi]
pub struct ExoSubstrateNode {
    inner: Arc<RwLock<SubstrateInstance>>,
}

#[napi]
impl ExoSubstrateNode {
    #[napi(constructor)]
    pub fn new(config: serde_json::Value) -> Result<Self> {
        let config: SubstrateConfig = serde_json::from_value(config)?;
        let inner = SubstrateInstance::new(config)?;
        Ok(Self { inner: Arc::new(RwLock::new(inner)) })
    }

    #[napi]
    pub async fn search(&self, embedding: Float32Array, k: u32) -> Result<Vec<SearchResultJs>> {
        let guard = self.inner.read().await;
        let results = guard.search(
            Query::from_embedding(embedding.to_vec()),
            k as usize,
        ).await?;
        Ok(results.into_iter().map(SearchResultJs::from).collect())
    }

    #[napi]
    pub async fn hypergraph_query(&self, query: String) -> Result<serde_json::Value> {
        let guard = self.inner.read().await;
        let topo_query: TopologicalQuery = serde_json::from_str(&query)?;
        let result = guard.hypergraph.query(&topo_query).await?;
        Ok(serde_json::to_value(result)?)
    }
}
```

---

## 5. Deployment Targets

### 5.1 Build Configurations

```toml
# Cargo.toml feature flags

[features]
default = ["classical-backend"]

# Backends
classical-backend = ["ruvector-core", "ruvector-graph", "ruvector-gnn"]
sim-neuromorphic = []
sim-photonic = []

# Deployment targets
wasm = ["wasm-bindgen", "getrandom/js"]
napi = ["napi", "napi-derive"]

# Experimental features
tensor-train = []
sheaf-consistency = []
post-quantum = ["kyberlib", "pqcrypto"]
```

### 5.2 Platform Matrix

| Target | Backend | Features | Size |
|--------|---------|----------|------|
| `wasm32-unknown-unknown` | Classical (memory-only) | Core substrate | ~2MB |
| `x86_64-unknown-linux-gnu` | Classical (full) | All features | ~15MB |
| `aarch64-apple-darwin` | Classical (full) | All features | ~12MB |
| Node.js (NAPI) | Classical (full) | All features | ~8MB |

---

## 6. Future Architecture Extensions

### 6.1 PIM Integration Path

```
Phase 1: Abstraction (Current)
├── Define PimBackend trait
├── Implement simulation mode
└── Profile classical baseline

Phase 2: Emulation
├── UPMEM SDK integration
├── Performance modeling
└── Hybrid execution strategies

Phase 3: Native Hardware
├── Custom PIM firmware
├── Memory bank allocation
└── Direct execution path
```

### 6.2 Consciousness Metrics (Research)

```rust
//! Experimental: Integrated Information metrics

/// Compute Phi (integrated information) for substrate region
pub fn compute_phi(
    substrate: &SubstrateRegion,
    partition_strategy: PartitionStrategy,
) -> f64 {
    // Compute information generated by whole
    let whole_info = substrate.effective_information();

    // Compute information generated by parts
    let partitions = partition_strategy.partition(substrate);
    let parts_info: f64 = partitions
        .iter()
        .map(|p| p.effective_information())
        .sum();

    // Phi = whole - parts (simplified IIT measure)
    (whole_info - parts_info).max(0.0)
}
```

---

## References

- SPARC Specification: `specs/SPECIFICATION.md`
- Research Papers: `research/PAPERS.md`
- Rust Libraries: `research/RUST_LIBRARIES.md`
