# Coherence Engine: Domain-Driven Design

**Version**: 0.3
**Date**: 2026-01-22
**Status**: Draft

---

## Strategic Design

### Domain Vision

The Coherence Engine provides a **continuously updated field of coherence** that shows where action is safe and where action must stop. It replaces probabilistic confidence with mathematical witnesses based on sheaf Laplacian theory.

> **This is not prediction.** The system answers: "Does the world still fit together?" not "What will happen?"

### The Universal Coherence Object

The power lies in a **single underlying coherence object** inside ruvector. Once the math is fixed, everything else becomes interpretation:

| Domain | Nodes Become | Edges Become | Residual Becomes | Gate Becomes |
|--------|--------------|--------------|------------------|--------------|
| **AI Agents** | Facts, hypotheses, beliefs | Citations, logical implication | Contradiction energy | Hallucination refusal |
| **Finance** | Trades, positions, signals | Market dependencies, arbitrage | Regime mismatch | Trading throttle |
| **Medical** | Vitals, diagnoses, treatments | Physiological causality | Clinical disagreement | Escalation trigger |
| **Robotics** | Sensor readings, goals, plans | Physics, kinematics | Motion impossibility | Safety stop |
| **Security** | Identities, permissions, actions | Policy rules, trust chains | Authorization violation | Access denial |
| **Science** | Hypotheses, observations, models | Experimental evidence | Theory inconsistency | Pruning signal |

**Same math, different interpretations. Same residual = contradiction energy. Same gate = refusal mechanism with witness.**

### Core Domain

**Coherence Computation** - The heart of the system, computing edge residuals and aggregating them into global coherence energy scores. **Most updates stay in a low-latency reflex lane; sustained/growing incoherence triggers escalation.**

### Supporting Domains

1. **Knowledge Substrate** - Graph state management (nodes, edges, restriction maps)
2. **Governance** - Policy, witness, and lineage management (signed policy bundles, mandatory witnesses)
3. **Action Execution** - Gated side effects with mandatory witnesses (refusal with proof)
4. **Tile Fabric** - 256-tile WASM distributed computation (cognitum-gate-kernel)
5. **Neural Gating** - Biologically-inspired compute ladder (ruvector-nervous-system)
6. **Adaptive Learning** - Self-optimizing thresholds from real traces (sona)

### Generic Domains

1. **Storage** - PostgreSQL (transactional authority) + ruvector (high-performance vector/graph)
2. **Event Sourcing** - Deterministic replay from signed event log
3. **Distributed Consensus** - Multi-node synchronization (ruvector-raft)

### Application Evolution

The universal coherence object enables a clean spectrum of applications without rewriting the core:

| Timeline | Applications | Key Capability |
|----------|-------------|----------------|
| **Today** | Anti-hallucination guards, market regime throttles, audit-ready compliance proofs | Structural proof, not probability |
| **Next (12-24mo)** | Safety-first drone autonomy, medical monitoring (sustained disagreement), zero-trust security | Proactive detection before alerts |
| **Future (5-10yr)** | Scientific theory pruning, policy stress testing, **self-awareness primitive** | System knows when it doesn't know |

> **Self-Awareness Primitive**: The system eventually knows when it no longer understands itself.

---

## Ruvector Ecosystem Integration Map

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              BOUNDED CONTEXTS                                    │
│                                                                                  │
│  ┌─────────────────────────────────────────────────────────────────────────────┐│
│  │                        TILE FABRIC (cognitum-gate-kernel)                   ││
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐        ┌─────────┐                    ││
│  │  │ Tile 0  │ │ Tile 1  │ │ Tile 2  │  ...   │Tile 255 │                    ││
│  │  │ Shard   │ │ Shard   │ │ Shard   │        │ Shard   │                    ││
│  │  │ E-value │ │ E-value │ │ E-value │        │ E-value │                    ││
│  │  │ Witness │ │ Witness │ │ Witness │        │ Witness │                    ││
│  │  └─────────┘ └─────────┘ └─────────┘        └─────────┘                    ││
│  └─────────────────────────────────────────────────────────────────────────────┘│
│                                      │                                          │
│                                      ▼                                          │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐              │
│  │ COHERENCE        │  │ KNOWLEDGE        │  │ NEURAL GATING    │              │
│  │ COMPUTATION      │◀─│ SUBSTRATE        │──│ (nervous-system) │              │
│  │                  │  │                  │  │                  │              │
│  │ • Energy calc    │  │ • SheafGraph     │  │ • CoherenceGated │              │
│  │ • Spectral       │  │ • Learned ρ(GNN) │  │ • GlobalWorkspace│              │
│  │ • Hyperbolic     │  │ • Hyperbolic idx │  │ • HDC witnesses  │              │
│  │ • Attention wgt  │  │ • MinCut isolate │  │ • Dendrites      │              │
│  └──────────────────┘  └──────────────────┘  └──────────────────┘              │
│           │                    │                      │                         │
│           └────────────────────┼──────────────────────┘                         │
│                                ▼                                                │
│  ┌──────────────────────────────────────────────────────────────────────────┐  │
│  │                    ADAPTIVE LEARNING (sona)                               │  │
│  │  Micro-LoRA (instant) │ Base-LoRA (background) │ EWC++ (no forgetting)   │  │
│  │  ReasoningBank        │ Three learning loops    │ Pattern extraction      │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
│                                │                                                │
│                                ▼                                                │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐              │
│  │ GOVERNANCE       │  │ ACTION           │  │ DISTRIBUTED      │              │
│  │                  │  │ EXECUTION        │  │ CONSENSUS (raft) │              │
│  │ • PolicyBundle   │  │ • Gate           │  │ • Leader elect   │              │
│  │ • WitnessRecord  │  │ • ComputeLadder  │  │ • Log replicate  │              │
│  │ • LineageRecord  │  │ • Escalation     │  │ • Global energy  │              │
│  └──────────────────┘  └──────────────────┘  └──────────────────┘              │
│                                                                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### Crate-to-Context Mapping

| Bounded Context | Primary Crate | Supporting Crates |
|-----------------|---------------|-------------------|
| Tile Fabric | `cognitum-gate-kernel` | - |
| Coherence Computation | `ruvector-coherence` (new) | `ruvector-attention`, `ruvector-hyperbolic-hnsw` |
| Knowledge Substrate | `ruvector-graph` | `ruvector-gnn`, `ruvector-mincut`, `ruvector-core` |
| Neural Gating | `ruvector-nervous-system` | - |
| Adaptive Learning | `sona` | `ruvector-gnn` (EWC) |
| Governance | `ruvector-coherence` (new) | - |
| Action Execution | `ruvector-coherence` (new) | `ruvector-nervous-system` |
| Distributed Consensus | `ruvector-raft` | - |

---

## Bounded Contexts

### Context Map

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              COHERENCE ENGINE                                │
│                                                                              │
│  ┌──────────────────┐      ┌──────────────────┐      ┌──────────────────┐  │
│  │                  │      │                  │      │                  │  │
│  │  SIGNAL          │─────▶│  KNOWLEDGE       │─────▶│  COHERENCE       │  │
│  │  INGESTION       │      │  SUBSTRATE       │      │  COMPUTATION     │  │
│  │                  │      │                  │      │                  │  │
│  └──────────────────┘      └──────────────────┘      └────────┬─────────┘  │
│           │                         │                          │            │
│           │                         │                          │            │
│           ▼                         ▼                          ▼            │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                           GOVERNANCE                                  │  │
│  │  Policy Bundles │ Witness Records │ Lineage Records │ Audit Trail    │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                      │                                      │
│                                      ▼                                      │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                        ACTION EXECUTION                               │  │
│  │  Coherence Gate │ Compute Ladder │ Escalation │ Side Effects         │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘

Context Relationships:
─────▶  Upstream/Downstream (Published Language)
```

---

## Bounded Context 0: Tile Fabric (cognitum-gate-kernel)

### Purpose

Provides the distributed computation substrate using 256 WASM tiles, each maintaining a local graph shard with evidence accumulation and witness fragments.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Tile** | A ~64KB WASM kernel instance processing a graph shard |
| **Delta** | Incremental update (edge add/remove, observation, weight change) |
| **E-Value** | Evidence value for sequential hypothesis testing |
| **Witness Fragment** | Local contribution to global min-cut witness |
| **Tick** | One deterministic processing cycle of a tile |

### Aggregates (from cognitum-gate-kernel)

#### TileState (Aggregate Root)

```rust
// Directly from cognitum-gate-kernel
use cognitum_gate_kernel::{TileState, Delta, WitnessFragment, TileReport};

/// Adapter for coherence engine integration
pub struct CoherenceTile {
    /// The underlying tile state
    inner: TileState,
    /// Mapping to global node IDs
    node_map: HashMap<LocalNodeId, GlobalNodeId>,
}

impl CoherenceTile {
    /// Create a new coherence tile
    pub fn new(tile_id: u8) -> Self {
        Self {
            inner: TileState::new(tile_id),
            node_map: HashMap::new(),
        }
    }

    /// Ingest a sheaf graph update as a tile delta
    pub fn ingest_node_update(&mut self, node_id: GlobalNodeId, state: &[f32]) -> bool {
        let local_id = self.node_map.entry(node_id)
            .or_insert_with(|| self.allocate_local_id());

        let delta = Delta::observation(Observation::state_update(*local_id, state));
        self.inner.ingest_delta(&delta)
    }

    /// Execute tick and return report
    pub fn tick(&mut self, tick_number: u32) -> TileReport {
        self.inner.tick(tick_number)
    }

    /// Get witness fragment for aggregation
    pub fn witness_fragment(&self) -> WitnessFragment {
        self.inner.get_witness_fragment()
    }

    /// Get current e-value (evidence against coherence hypothesis)
    pub fn e_value(&self) -> f64 {
        self.inner.evidence.global_e_value()
    }
}
```

#### TileFabric (Domain Service)

```rust
/// Orchestrates 256 tiles for distributed coherence
pub struct TileFabric {
    tiles: Vec<CoherenceTile>,
    shard_strategy: ShardStrategy,
}

impl TileFabric {
    /// Create fabric with 256 tiles
    pub fn new(strategy: ShardStrategy) -> Self {
        Self {
            tiles: (0..256).map(|i| CoherenceTile::new(i as u8)).collect(),
            shard_strategy: strategy,
        }
    }

    /// Distribute node to appropriate tile based on sharding strategy
    pub fn route_node(&self, node_id: GlobalNodeId) -> u8 {
        self.shard_strategy.tile_for(node_id)
    }

    /// Parallel tick across all tiles
    pub fn tick_all(&mut self, tick_number: u32) -> FabricReport {
        let reports: Vec<TileReport> = self.tiles
            .par_iter_mut()
            .map(|tile| tile.tick(tick_number))
            .collect();

        FabricReport::aggregate(reports)
    }

    /// Collect all witness fragments
    pub fn collect_witnesses(&self) -> Vec<WitnessFragment> {
        self.tiles.iter()
            .map(|t| t.witness_fragment())
            .collect()
    }
}
```

### Domain Events

| Event | Trigger | Consumers |
|-------|---------|-----------|
| `DeltaIngested` | Tile receives update | Tile processing |
| `TickCompleted` | Tile finishes tick | Fabric aggregation |
| `WitnessGenerated` | Tick produces witness | Global aggregation |
| `EvidenceThresholdCrossed` | E-value exceeds limit | Escalation |

---

## Bounded Context 0.5: Adaptive Learning (sona)

### Purpose

Provides self-optimizing threshold tuning using SONA's three learning loops, with EWC++ to prevent catastrophic forgetting when adapting to new operational regimes.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Trajectory** | A sequence of energy observations during an operational regime |
| **Micro-LoRA** | Ultra-low rank (1-2) adaptation for instant learning (<0.05ms) |
| **Base-LoRA** | Standard LoRA for background learning |
| **EWC++** | Elastic Weight Consolidation preventing catastrophic forgetting |
| **ReasoningBank** | Pattern storage for past successful configurations |

### Aggregates

#### ThresholdLearner (Aggregate Root)

```rust
use sona::{
    SonaEngine, SonaConfig, TrajectoryBuilder, TrajectoryStep,
    MicroLoRA, BaseLoRA, EwcPlusPlus, ReasoningBank,
};

/// Adaptive threshold learning using SONA
pub struct ThresholdLearner {
    /// SONA engine
    engine: SonaEngine,
    /// Current thresholds
    thresholds: ThresholdConfig,
    /// Active trajectory (if any)
    active_trajectory: Option<TrajectoryBuilder>,
    /// Pattern bank for successful configurations
    patterns: ReasoningBank,
}

impl ThresholdLearner {
    pub fn new(hidden_dim: usize) -> Self {
        let config = SonaConfig {
            hidden_dim,
            embedding_dim: hidden_dim,
            ..Default::default()
        };

        Self {
            engine: SonaEngine::new(config),
            thresholds: ThresholdConfig::default(),
            active_trajectory: None,
            patterns: ReasoningBank::new(PatternConfig::default()),
        }
    }

    /// Start learning when entering new regime
    pub fn begin_regime(&mut self, initial_energy: Vec<f32>) {
        self.active_trajectory = Some(self.engine.begin_trajectory(initial_energy));
    }

    /// Record an observation during the regime
    pub fn observe(&mut self, energy: Vec<f32>, action_taken: Vec<f32>, quality: f32) {
        if let Some(ref mut traj) = self.active_trajectory {
            traj.add_step(energy, action_taken, quality);
        }
    }

    /// End regime and learn from outcome
    pub fn end_regime(&mut self, final_quality: f32) -> DomainEvent {
        if let Some(traj) = self.active_trajectory.take() {
            // Triggers Micro-LoRA instant adaptation
            self.engine.end_trajectory(traj, final_quality);

            if final_quality > 0.8 {
                // Store successful pattern
                self.patterns.store(
                    PatternType::Threshold,
                    &self.thresholds.to_embedding(),
                );
                return DomainEvent::PatternLearned { quality: final_quality };
            }
        }
        DomainEvent::RegimeEnded { quality: final_quality }
    }

    /// Find similar past regime
    pub fn recall_similar(&self, current_energy: &[f32]) -> Option<ThresholdConfig> {
        self.patterns.query(current_energy, 5)
            .first()
            .map(|p| ThresholdConfig::from_embedding(&p.embedding))
    }

    /// Consolidate to prevent forgetting
    pub fn consolidate(&mut self) {
        // EWC++ preserves important weights
        self.engine.consolidate_ewc();
    }

    /// Apply instant adaptation (Micro-LoRA)
    pub fn instant_adapt(&mut self, energy_spike: f32) -> ThresholdAdjustment {
        let input = vec![energy_spike; self.engine.config().hidden_dim];
        let mut output = vec![0.0; self.engine.config().hidden_dim];

        self.engine.apply_micro_lora(&input, &mut output);

        ThresholdAdjustment::from_embedding(&output)
    }
}
```

### Domain Events

| Event | Trigger | Consumers |
|-------|---------|-----------|
| `RegimeStarted` | New operational regime detected | TrajectoryBuilder |
| `ObservationRecorded` | Energy observation added | Active trajectory |
| `RegimeEnded` | Regime completed | Learning consolidation |
| `PatternLearned` | Successful pattern stored | ReasoningBank |
| `ThresholdAdapted` | Micro-LoRA adaptation | Gate configuration |

---

## Bounded Context 0.7: Neural Gating (ruvector-nervous-system)

### Purpose

Provides biologically-inspired gating using the existing CoherenceGatedSystem, GlobalWorkspace for conscious access, and HDC for compact witness encoding.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **CoherenceGatedSystem** | Pre-existing neural gating from ruvector-nervous-system |
| **GlobalWorkspace** | Conscious broadcast mechanism for significant decisions |
| **Hypervector** | 10K-dimensional binary vector for similarity-preserving encoding |
| **Dendrite** | Coincidence detector requiring multiple inputs within time window |
| **Plateau Potential** | Threshold firing when dendritic conditions met |

### Aggregates

#### NeuralGate (Adapter to existing CoherenceGatedSystem)

```rust
use ruvector_nervous_system::{
    CoherenceGatedSystem, GlobalWorkspace, HysteresisTracker,
    OscillatoryRouter, Dendrite, DendriticTree,
    HdcMemory, Hypervector,
};

/// Neural gating using existing ruvector-nervous-system
pub struct NeuralGate {
    /// The existing coherence-gated system
    system: CoherenceGatedSystem,
    /// Global workspace for broadcast
    workspace: GlobalWorkspace,
    /// Hysteresis to prevent oscillation
    hysteresis: HysteresisTracker,
    /// Dendritic coincidence detection
    dendrites: DendriticTree,
    /// HDC memory for witnesses
    hdc: HdcMemory,
}

impl NeuralGate {
    /// Evaluate with biological gating
    pub fn evaluate(&mut self, energy: f32, evidence: &[EvidenceSource]) -> NeuralDecision {
        // Feed evidence to dendrites
        for (i, src) in evidence.iter().enumerate() {
            if src.is_active() {
                self.dendrites.receive_spike(i, src.timestamp);
            }
        }

        // Check for plateau potential (coincidence detection)
        let plateau = self.dendrites.update(evidence[0].timestamp, 1.0);

        // Apply hysteresis to prevent oscillation
        let decision = self.hysteresis.filter(energy, plateau);

        // Broadcast significant decisions
        if decision.is_significant() {
            self.workspace.broadcast(decision.clone());
        }

        decision
    }

    /// Encode witness as hypervector
    pub fn encode_witness(&mut self, witness: &WitnessRecord) -> WitnessHypervector {
        let energy_hv = Hypervector::from_scalar(witness.energy_snapshot.total_energy);
        let decision_hv = Hypervector::random(); // Seed from decision
        let policy_hv = Hypervector::from_bytes(&witness.policy_bundle_ref.as_bytes());

        let encoded = energy_hv.bind(&decision_hv).bind(&policy_hv);

        self.hdc.store(&witness.id.to_string(), encoded.clone());

        WitnessHypervector(encoded)
    }

    /// Find similar past decisions
    pub fn find_similar(&self, query: &Hypervector, threshold: f32) -> Vec<WitnessId> {
        self.hdc.retrieve(query, threshold)
            .into_iter()
            .filter_map(|(id, _)| WitnessId::parse(&id).ok())
            .collect()
    }
}
```

### Domain Events

| Event | Trigger | Consumers |
|-------|---------|-----------|
| `PlateauTriggered` | Dendritic coincidence | Decision evaluation |
| `DecisionBroadcast` | Significant decision | GlobalWorkspace subscribers |
| `WitnessEncoded` | HDC encoding complete | Similarity search |

---

## Bounded Context 1: Signal Ingestion

### Purpose

Validates and normalizes incoming events before they enter the knowledge substrate.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Signal** | Raw incoming event from external system |
| **Normalized Event** | Validated, typed event ready for processing |
| **Event Schema** | Contract defining valid event structure |
| **Ingestion Pipeline** | Sequence of validation and transformation steps |

### Aggregates

#### SignalProcessor

```rust
/// Root aggregate for signal processing
pub struct SignalProcessor {
    id: ProcessorId,
    schemas: HashMap<EventType, EventSchema>,
    validators: Vec<Box<dyn Validator>>,
    transformers: Vec<Box<dyn Transformer>>,
}

impl SignalProcessor {
    /// Process a raw signal into a normalized event
    pub fn process(&self, signal: RawSignal) -> Result<NormalizedEvent, IngestionError> {
        // Validate against schema
        let schema = self.schemas.get(&signal.event_type)
            .ok_or(IngestionError::UnknownEventType)?;
        schema.validate(&signal)?;

        // Run validators
        for validator in &self.validators {
            validator.validate(&signal)?;
        }

        // Transform to normalized form
        let mut event = NormalizedEvent::from(signal);
        for transformer in &self.transformers {
            event = transformer.transform(event)?;
        }

        Ok(event)
    }
}
```

### Domain Events

| Event | Trigger | Consumers |
|-------|---------|-----------|
| `SignalReceived` | External signal arrives | SignalProcessor |
| `EventNormalized` | Validation passes | Knowledge Substrate |
| `SignalRejected` | Validation fails | Monitoring, Alerting |

### Integration Patterns

- **Anti-Corruption Layer**: Translates external formats to internal domain model
- **Published Language**: JSON Schema for event contracts

---

## Bounded Context 2: Knowledge Substrate

### Purpose

Maintains the sheaf graph representing system state with nodes, edges, and restriction maps. **The same substrate serves all domains** - nodes can be facts, trades, vitals, devices, hypotheses, or beliefs; edges can encode citations, causality, physiology, policy, or physics.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Sheaf Node** | Entity with fixed-dimensional state vector (domain-agnostic: facts, trades, vitals, hypotheses, beliefs) |
| **Sheaf Edge** | Constraint between two nodes via restriction maps (domain-agnostic: citations, causality, physiology, policy, physics) |
| **Restriction Map** | Lightweight linear transformation encoding how one state constrains another |
| **Stalk** | The state vector at a node (sheaf terminology) |
| **Section** | A consistent assignment of states to nodes |
| **Residual** | **Contradiction energy** - mismatch between projected states |

### Aggregates

#### SheafGraph (Aggregate Root)

```rust
/// The sheaf graph representing system state
pub struct SheafGraph {
    id: GraphId,
    nodes: HashMap<NodeId, SheafNode>,
    edges: HashMap<EdgeId, SheafEdge>,
    namespaces: HashMap<NamespaceId, NodeSet>,
    version: Version,
    fingerprint: Hash,
}

impl SheafGraph {
    /// Add or update a node's state
    pub fn upsert_node(&mut self, node: SheafNode) -> DomainEvent {
        let existed = self.nodes.insert(node.id, node.clone()).is_some();
        self.update_fingerprint();

        if existed {
            DomainEvent::NodeUpdated { node_id: node.id }
        } else {
            DomainEvent::NodeCreated { node_id: node.id }
        }
    }

    /// Add an edge with restriction maps
    pub fn add_edge(&mut self, edge: SheafEdge) -> Result<DomainEvent, DomainError> {
        // Validate nodes exist
        if !self.nodes.contains_key(&edge.source) {
            return Err(DomainError::NodeNotFound(edge.source));
        }
        if !self.nodes.contains_key(&edge.target) {
            return Err(DomainError::NodeNotFound(edge.target));
        }

        // Validate dimension compatibility
        let source_dim = self.nodes[&edge.source].state.len();
        let target_dim = self.nodes[&edge.target].state.len();

        if edge.rho_source.input_dim() != source_dim {
            return Err(DomainError::DimensionMismatch);
        }
        if edge.rho_target.input_dim() != target_dim {
            return Err(DomainError::DimensionMismatch);
        }

        self.edges.insert(edge.id, edge.clone());
        self.update_fingerprint();

        Ok(DomainEvent::EdgeCreated { edge_id: edge.id })
    }

    /// Get subgraph for localized computation
    pub fn subgraph(&self, center: NodeId, hops: usize) -> SheafSubgraph {
        let node_ids = self.bfs_neighbors(center, hops);
        SheafSubgraph {
            nodes: node_ids.iter()
                .filter_map(|id| self.nodes.get(id).cloned())
                .collect(),
            edges: self.edges.values()
                .filter(|e| node_ids.contains(&e.source) && node_ids.contains(&e.target))
                .cloned()
                .collect(),
        }
    }
}
```

#### SheafNode (Entity)

```rust
/// A node in the sheaf graph
pub struct SheafNode {
    id: NodeId,
    state: Vec<f32>,
    metadata: NodeMetadata,
    namespace: NamespaceId,
    created_at: Timestamp,
    updated_at: Timestamp,
}

impl SheafNode {
    /// Invariant: state dimension is fixed after creation
    pub fn update_state(&mut self, new_state: Vec<f32>) -> Result<(), DomainError> {
        if new_state.len() != self.state.len() {
            return Err(DomainError::DimensionMismatch);
        }
        self.state = new_state;
        self.updated_at = Timestamp::now();
        Ok(())
    }
}
```

#### RestrictionMap (Value Object)

```rust
/// Linear restriction map: y = Ax + b
#[derive(Clone, PartialEq)]
pub struct RestrictionMap {
    matrix: Matrix,  // m x n where n = input_dim, m = output_dim
    bias: Vec<f32>,  // m-dimensional
}

impl RestrictionMap {
    pub fn new(matrix: Matrix, bias: Vec<f32>) -> Result<Self, DomainError> {
        if matrix.nrows() != bias.len() {
            return Err(DomainError::DimensionMismatch);
        }
        Ok(Self { matrix, bias })
    }

    pub fn apply(&self, input: &[f32]) -> Vec<f32> {
        let result = &self.matrix * &DVector::from_slice(input);
        result.iter()
            .zip(self.bias.iter())
            .map(|(a, b)| a + b)
            .collect()
    }

    pub fn input_dim(&self) -> usize { self.matrix.ncols() }
    pub fn output_dim(&self) -> usize { self.matrix.nrows() }
}
```

### Domain Events

| Event | Trigger | Consumers |
|-------|---------|-----------|
| `NodeCreated` | New node added | Coherence Computation |
| `NodeUpdated` | Node state changed | Coherence Computation (incremental) |
| `EdgeCreated` | New constraint added | Coherence Computation |
| `SubgraphExtracted` | Localized computation needed | Coherence Computation |

### Repository Interface

```rust
#[async_trait]
pub trait SheafGraphRepository {
    async fn find_by_id(&self, id: GraphId) -> Option<SheafGraph>;
    async fn save(&self, graph: &SheafGraph) -> Result<(), PersistenceError>;
    async fn find_nodes_by_namespace(&self, ns: NamespaceId) -> Vec<SheafNode>;
    async fn find_similar_nodes(&self, state: &[f32], k: usize) -> Vec<(NodeId, f32)>;
}
```

---

## Bounded Context 3: Coherence Computation

### Purpose

Computes edge residuals, aggregates energy, and detects structural inconsistencies. This maintains a **continuously updated field of coherence** that shows where action is safe and where action must stop.

> **Key Principle**: Incoherence is computed incrementally as edge-level residuals and aggregated into scoped coherence energy using a sheaf Laplacian operator.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Residual** | **Contradiction energy** at an edge: r_e = ρ_u(x_u) - ρ_v(x_v) |
| **Energy** | Global incoherence measure: E = Σ w_e\|r_e\|² (lower = more coherent) |
| **Coherence Field** | Continuously updated map showing safe vs. unsafe action regions |
| **Coherence** | Inverse of energy; high coherence = low energy |
| **Fingerprint** | Hash summarizing graph state for change detection |
| **Spectral Drift** | Change in eigenvalue distribution indicating structural shift |

### Aggregates

#### CoherenceEngine (Aggregate Root)

```rust
/// The coherence computation engine
pub struct CoherenceEngine {
    id: EngineId,
    /// Cached residuals for incremental computation
    residual_cache: HashMap<EdgeId, Vec<f32>>,
    /// Subgraph energy summaries
    summary_cache: HashMap<SubgraphId, f32>,
    /// Global fingerprint
    fingerprint: Hash,
    /// Configuration
    config: CoherenceConfig,
}

impl CoherenceEngine {
    /// Compute full coherence energy for a graph
    pub fn compute_energy(&mut self, graph: &SheafGraph) -> CoherenceEnergy {
        // Check if we can use incremental computation
        if self.fingerprint == graph.fingerprint {
            return self.cached_energy();
        }

        // Full recomputation
        let edge_energies: HashMap<EdgeId, f32> = graph.edges
            .par_iter()
            .map(|(id, edge)| {
                let residual = self.compute_residual(graph, edge);
                let energy = edge.weight * residual.iter().map(|x| x * x).sum::<f32>();
                self.residual_cache.insert(*id, residual);
                (*id, energy)
            })
            .collect();

        let total = edge_energies.values().sum();
        self.fingerprint = graph.fingerprint;

        CoherenceEnergy {
            total_energy: total,
            edge_energies,
            scope_energies: self.aggregate_by_scope(graph, &edge_energies),
            fingerprint: self.fingerprint,
            computed_at: Timestamp::now(),
        }
    }

    /// Incremental update when a single node changes
    pub fn update_node(&mut self, graph: &SheafGraph, node_id: NodeId) -> CoherenceEnergy {
        let affected_edges = graph.edges_incident_to(node_id);

        for edge_id in &affected_edges {
            let edge = &graph.edges[edge_id];
            let residual = self.compute_residual(graph, edge);
            self.residual_cache.insert(*edge_id, residual);
        }

        self.recompute_from_cache(graph)
    }

    fn compute_residual(&self, graph: &SheafGraph, edge: &SheafEdge) -> Vec<f32> {
        let source_state = &graph.nodes[&edge.source].state;
        let target_state = &graph.nodes[&edge.target].state;

        let projected_source = edge.rho_source.apply(source_state);
        let projected_target = edge.rho_target.apply(target_state);

        projected_source.iter()
            .zip(projected_target.iter())
            .map(|(a, b)| a - b)
            .collect()
    }
}
```

#### CoherenceEnergy (Value Object)

```rust
/// Immutable snapshot of coherence energy
#[derive(Clone)]
pub struct CoherenceEnergy {
    pub total_energy: f32,
    pub edge_energies: HashMap<EdgeId, f32>,
    pub scope_energies: HashMap<ScopeId, f32>,
    pub fingerprint: Hash,
    pub computed_at: Timestamp,
}

impl CoherenceEnergy {
    /// Get energy for a specific scope
    pub fn scope_energy(&self, scope: &ScopeId) -> f32 {
        self.scope_energies.get(scope).copied().unwrap_or(0.0)
    }

    /// Find edges with highest energy (most incoherent)
    pub fn hotspots(&self, k: usize) -> Vec<(EdgeId, f32)> {
        let mut sorted: Vec<_> = self.edge_energies.iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
        sorted.into_iter().take(k).map(|(id, e)| (*id, *e)).collect()
    }
}
```

### Domain Services

#### SpectralAnalyzer

```rust
/// Detects structural drift via eigenvalue analysis
pub struct SpectralAnalyzer {
    history: VecDeque<EigenvalueSnapshot>,
    drift_threshold: f32,
    window_size: usize,
}

impl SpectralAnalyzer {
    /// Analyze eigenvalues for drift detection
    pub fn analyze(&mut self, laplacian: &SheafLaplacian) -> SpectralAnalysis {
        let eigenvalues = laplacian.compute_top_eigenvalues(10);
        let snapshot = EigenvalueSnapshot::new(eigenvalues.clone());

        let drift = if let Some(prev) = self.history.back() {
            self.wasserstein_distance(&snapshot.eigenvalues, &prev.eigenvalues)
        } else {
            0.0
        };

        self.history.push_back(snapshot);
        if self.history.len() > self.window_size {
            self.history.pop_front();
        }

        SpectralAnalysis {
            eigenvalues,
            drift_magnitude: drift,
            is_drifting: drift > self.drift_threshold,
            timestamp: Timestamp::now(),
        }
    }
}
```

### Domain Events

| Event | Trigger | Consumers |
|-------|---------|-----------|
| `EnergyComputed` | Full computation completes | Governance, Gate |
| `EnergyUpdated` | Incremental update completes | Governance, Gate |
| `DriftDetected` | Spectral drift exceeds threshold | Alerting, Escalation |
| `HotspotIdentified` | Edge energy exceeds threshold | Debugging, Monitoring |

---

## Bounded Context 4: Governance

### Purpose

Manages policy bundles, witness records, and lineage tracking for auditability.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Policy Bundle** | Versioned, signed collection of threshold configurations |
| **Witness Record** | Immutable proof of a gate decision |
| **Lineage Record** | Provenance chain for authoritative writes |
| **Approver** | Entity authorized to sign policy bundles |
| **Threshold** | Energy level triggering escalation |

### Aggregates

#### PolicyBundle (Aggregate Root)

```rust
/// Versioned, multi-sig policy configuration
pub struct PolicyBundle {
    id: PolicyBundleId,
    version: SemanticVersion,
    thresholds: HashMap<ScopePattern, ThresholdConfig>,
    escalation_rules: Vec<EscalationRule>,
    signatures: Vec<(ApproverId, Signature)>,
    required_approvals: usize,
    status: PolicyStatus,
    created_at: Timestamp,
    activated_at: Option<Timestamp>,
}

impl PolicyBundle {
    /// Invariant: cannot modify after activation
    pub fn add_threshold(&mut self, scope: ScopePattern, config: ThresholdConfig) -> Result<(), DomainError> {
        if self.status != PolicyStatus::Draft {
            return Err(DomainError::PolicyAlreadyActivated);
        }
        self.thresholds.insert(scope, config);
        Ok(())
    }

    /// Add approver signature
    pub fn sign(&mut self, approver: ApproverId, signature: Signature) -> Result<DomainEvent, DomainError> {
        if self.status != PolicyStatus::Draft {
            return Err(DomainError::PolicyAlreadyActivated);
        }

        // Verify signature
        let content_hash = self.content_hash();
        if !signature.verify(&content_hash, &approver) {
            return Err(DomainError::InvalidSignature);
        }

        self.signatures.push((approver, signature));

        // Check if enough signatures
        if self.signatures.len() >= self.required_approvals {
            self.status = PolicyStatus::Approved;
            return Ok(DomainEvent::PolicyApproved { bundle_id: self.id });
        }

        Ok(DomainEvent::PolicySigned { bundle_id: self.id, approver })
    }

    /// Activate the policy (makes it immutable)
    pub fn activate(&mut self) -> Result<DomainEvent, DomainError> {
        if self.status != PolicyStatus::Approved {
            return Err(DomainError::PolicyNotApproved);
        }

        self.status = PolicyStatus::Active;
        self.activated_at = Some(Timestamp::now());

        Ok(DomainEvent::PolicyActivated { bundle_id: self.id })
    }
}
```

#### WitnessRecord (Entity)

```rust
/// Immutable record of a gate decision
pub struct WitnessRecord {
    id: WitnessId,
    action_hash: Hash,
    energy_snapshot: CoherenceEnergy,
    decision: GateDecision,
    policy_bundle_ref: PolicyBundleRef,
    timestamp: Timestamp,
    previous_witness: Option<WitnessId>,
    content_hash: Hash,
}

impl WitnessRecord {
    pub fn new(
        action: &Action,
        energy: &CoherenceEnergy,
        decision: GateDecision,
        policy_ref: PolicyBundleRef,
        previous: Option<WitnessId>,
    ) -> Self {
        let mut record = Self {
            id: WitnessId::new(),
            action_hash: action.content_hash(),
            energy_snapshot: energy.clone(),
            decision,
            policy_bundle_ref: policy_ref,
            timestamp: Timestamp::now(),
            previous_witness: previous,
            content_hash: Hash::default(),
        };
        record.content_hash = record.compute_content_hash();
        record
    }

    /// Content hash for integrity verification
    fn compute_content_hash(&self) -> Hash {
        let mut hasher = Blake3::new();
        hasher.update(&self.action_hash);
        hasher.update(&self.energy_snapshot.fingerprint);
        hasher.update(&bincode::serialize(&self.decision).unwrap());
        hasher.update(&self.policy_bundle_ref.as_bytes());
        if let Some(prev) = &self.previous_witness {
            hasher.update(&prev.as_bytes());
        }
        hasher.finalize().into()
    }

    /// Verify integrity
    pub fn verify(&self) -> bool {
        self.content_hash == self.compute_content_hash()
    }
}
```

#### LineageRecord (Entity)

```rust
/// Provenance tracking for writes
pub struct LineageRecord {
    id: LineageId,
    entity_ref: EntityRef,
    operation: Operation,
    dependencies: Vec<LineageId>,
    authorizing_witness: WitnessId,
    actor: ActorId,
    timestamp: Timestamp,
}

impl LineageRecord {
    /// Invariant: must have authorizing witness
    pub fn new(
        entity: EntityRef,
        operation: Operation,
        witness: WitnessId,
        actor: ActorId,
        dependencies: Vec<LineageId>,
    ) -> Self {
        Self {
            id: LineageId::new(),
            entity_ref: entity,
            operation,
            dependencies,
            authorizing_witness: witness,
            actor,
            timestamp: Timestamp::now(),
        }
    }
}
```

### Domain Events

| Event | Trigger | Consumers |
|-------|---------|-----------|
| `PolicyCreated` | New bundle drafted | Approvers |
| `PolicySigned` | Approver signs | Policy lifecycle |
| `PolicyApproved` | Enough signatures | Activation |
| `PolicyActivated` | Bundle goes live | Gate |
| `WitnessCreated` | Gate decision made | Audit, Lineage |
| `LineageCreated` | Write authorized | Audit |

### Invariants

1. **No action without witness**: Every external action must have a `witness_id`
2. **No write without lineage**: Every authoritative write must have a `lineage_id`
3. **Policy immutability**: Active policies cannot be modified
4. **Signature validity**: All policy signatures must verify against content hash
5. **Witness chain**: Each witness references its predecessor (except first)

---

## Bounded Context 5: Action Execution

### Purpose

Executes gated side effects with mandatory witness and lineage creation. A **deterministic coherence gate** controls a compute ladder where **most updates stay in a low-latency reflex lane**, while sustained/growing incoherence triggers retrieval, deeper reasoning, or human escalation.

> **Key Principle**: All decisions and external side effects are governed by **signed policy bundles** and produce **mandatory witness and lineage records**, making every action auditable and replayable.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Action** | External side effect to be executed |
| **Compute Lane** | Escalation level (Reflex → Retrieval → Heavy → Human) |
| **Reflex Lane** | **THE DEFAULT** - most updates stay here (<1ms) |
| **Gate Decision** | Allow/deny with required lane and witness |
| **Escalation** | Promotion to higher compute lane (triggered by **sustained** incoherence) |
| **Refusal** | Action denied due to incoherence - **refusal mechanism with witness** |
| **Witness** | Mandatory proof of every gate decision |

### Aggregates

#### CoherenceGate (Aggregate Root)

```rust
/// Gate controlling action execution
pub struct CoherenceGate {
    id: GateId,
    policy_bundle: PolicyBundle,
    energy_history: EnergyHistory,
    pending_escalations: HashMap<ActionId, Escalation>,
}

impl CoherenceGate {
    /// Evaluate whether an action should proceed
    pub fn evaluate(&mut self, action: &Action, energy: &CoherenceEnergy) -> GateDecision {
        let scope = action.scope();
        let current_energy = energy.scope_energy(&scope);

        // Get thresholds from policy
        let thresholds = self.policy_bundle.thresholds_for(&scope);

        // Determine required lane
        let lane = self.determine_lane(current_energy, &thresholds);

        // Check persistence
        let persistent = self.energy_history.is_persistently_above(
            &scope,
            thresholds.retrieval,
            thresholds.persistence_window,
        );

        // Escalate if persistent incoherence
        let final_lane = if persistent && lane < ComputeLane::Heavy {
            ComputeLane::Heavy
        } else {
            lane
        };

        // Record in history
        self.energy_history.record(scope.clone(), current_energy);

        GateDecision {
            allow: final_lane < ComputeLane::Human,
            lane: final_lane,
            reason: self.decision_reason(current_energy, &thresholds, persistent),
        }
    }

    fn determine_lane(&self, energy: f32, thresholds: &ThresholdConfig) -> ComputeLane {
        if energy < thresholds.reflex {
            ComputeLane::Reflex
        } else if energy < thresholds.retrieval {
            ComputeLane::Retrieval
        } else if energy < thresholds.heavy {
            ComputeLane::Heavy
        } else {
            ComputeLane::Human
        }
    }
}
```

#### ActionExecutor (Domain Service)

```rust
/// Executes actions with governance enforcement
pub struct ActionExecutor {
    gate: CoherenceGate,
    witness_repository: Arc<dyn WitnessRepository>,
    lineage_repository: Arc<dyn LineageRepository>,
}

impl ActionExecutor {
    /// Execute an action with full governance
    pub async fn execute<A: Action>(
        &mut self,
        action: A,
        energy: &CoherenceEnergy,
    ) -> Result<ExecutionResult<A::Output>, ExecutionError> {
        // Evaluate gate
        let decision = self.gate.evaluate(&action, energy);

        // Create witness (always, even for denials)
        let witness = WitnessRecord::new(
            &action,
            energy,
            decision.clone(),
            self.gate.policy_bundle.reference(),
            self.get_previous_witness().await,
        );
        self.witness_repository.save(&witness).await?;

        // Check if allowed
        if !decision.allow {
            return Err(ExecutionError::Denied {
                witness_id: witness.id,
                reason: decision.reason,
            });
        }

        // Execute the action
        let output = action.execute().await?;

        // Create lineage record for any writes
        if let Some(writes) = output.writes() {
            for write in writes {
                let lineage = LineageRecord::new(
                    write.entity_ref(),
                    write.operation(),
                    witness.id,
                    action.actor(),
                    write.dependencies(),
                );
                self.lineage_repository.save(&lineage).await?;
            }
        }

        Ok(ExecutionResult {
            output,
            witness_id: witness.id,
        })
    }
}
```

### Domain Events

| Event | Trigger | Consumers |
|-------|---------|-----------|
| `ActionAllowed` | Gate allows action | Executor, Monitoring |
| `ActionDenied` | Gate denies action | Alerting, Audit |
| `ActionExecuted` | Execution completes | Lineage, Monitoring |
| `EscalationTriggered` | Persistent incoherence | Human operators |

---

## Bounded Context 6: Learned Restriction Maps (ruvector-gnn)

### Purpose

Enables learning restriction maps (ρ) from data using GNN layers, with EWC to prevent catastrophic forgetting across training epochs.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **RuvectorLayer** | GNN layer implementing learned linear transformation |
| **Replay Buffer** | Experience replay for stable training |
| **Fisher Information** | Importance weight for EWC regularization |

### Aggregates

#### LearnedRestriction (Aggregate Root)

```rust
use ruvector_gnn::{
    RuvectorLayer, ElasticWeightConsolidation, ReplayBuffer,
    Optimizer, OptimizerType, LearningRateScheduler, SchedulerType,
    info_nce_loss, local_contrastive_loss,
};

/// Learned restriction map using GNN
pub struct LearnedRestriction {
    /// Neural network layer
    layer: RuvectorLayer,
    /// EWC for forgetting prevention
    ewc: ElasticWeightConsolidation,
    /// Experience replay
    replay: ReplayBuffer,
    /// Adam optimizer
    optimizer: Optimizer,
}

impl LearnedRestriction {
    /// Apply learned restriction
    pub fn apply(&self, state: &[f32]) -> Vec<f32> {
        self.layer.forward(state)
    }

    /// Train on coherent example pair
    pub fn train(&mut self, source: &[f32], target: &[f32], label: CoherenceLabel) {
        // Add to replay buffer
        self.replay.add(ReplayEntry {
            source: source.to_vec(),
            target: target.to_vec(),
            label,
        });

        // Sample batch
        let batch = self.replay.sample(32);

        // Compute contrastive loss
        let loss = local_contrastive_loss(&batch.embeddings(), &batch.labels(), 0.07);

        // Add EWC regularization
        let ewc_loss = self.ewc.compute_ewc_loss(&self.layer);
        let total_loss = loss + 0.4 * ewc_loss;

        // Update
        self.optimizer.step(&mut self.layer, total_loss);
    }

    /// Consolidate after epoch
    pub fn consolidate(&mut self, importance: f32) {
        self.ewc.update_fisher(&self.layer, importance);
    }
}
```

---

## Bounded Context 7: Hyperbolic Coherence (ruvector-hyperbolic-hnsw)

### Purpose

Provides hierarchy-aware energy computation where deeper nodes (further from origin in Poincaré ball) have higher coherence expectations.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Poincaré Ball** | Hyperbolic space model where distance increases toward boundary |
| **Depth** | Distance from origin; deeper = further from origin |
| **Curvature** | Negative curvature parameter (typically -1.0) |
| **Tangent Space** | Local Euclidean approximation for fast pruning |

### Aggregates

#### HyperbolicGraph (Aggregate Root)

```rust
use ruvector_hyperbolic_hnsw::{
    HyperbolicHnsw, HyperbolicHnswConfig, ShardedHyperbolicHnsw,
    poincare_distance, project_to_ball, log_map, exp_map,
    HierarchyMetrics, TangentCache,
};

/// Hyperbolic coherence with hierarchy awareness
pub struct HyperbolicGraph {
    /// Hyperbolic index
    index: ShardedHyperbolicHnsw,
    /// Curvature
    curvature: f32,
    /// Tangent cache for fast pruning
    tangent_cache: TangentCache,
}

impl HyperbolicGraph {
    /// Insert node with automatic depth assignment
    pub fn insert(&mut self, node_id: NodeId, state: Vec<f32>, hierarchy_depth: Option<usize>) {
        let projected = project_to_ball(&state, self.curvature);
        self.index.insert(projected, hierarchy_depth).unwrap();
    }

    /// Compute depth-weighted residual energy
    pub fn weighted_residual(&self, edge: &SheafEdge, residual: &[f32]) -> f32 {
        let source_depth = self.depth(&edge.source);
        let target_depth = self.depth(&edge.target);

        // Deeper nodes should be MORE coherent, so weight violations higher
        let depth_weight = 1.0 + ((source_depth + target_depth) / 2.0).ln().max(0.0);

        let norm_sq: f32 = residual.iter().map(|x| x * x).sum();
        edge.weight * norm_sq * depth_weight
    }

    /// Compute node depth (distance from origin)
    fn depth(&self, node_id: &NodeId) -> f32 {
        let state = self.index.get(node_id);
        let origin = vec![0.0; state.len()];
        poincare_distance(&state, &origin, self.curvature)
    }

    /// Build tangent cache for fast neighbor search
    pub fn build_tangent_cache(&mut self) {
        self.index.build_tangent_cache().unwrap();
    }
}
```

---

## Bounded Context 8: Incoherence Isolation (ruvector-mincut)

### Purpose

Efficiently isolates incoherent subgraphs using subpolynomial n^o(1) dynamic min-cut algorithms.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **MinCut** | Minimum weight edge set whose removal disconnects graph |
| **Subpolynomial** | Update time n^o(1), faster than any polynomial |
| **Witness Tree** | Proof structure for cut validity |
| **Cognitive Engine** | SNN-based optimization for cuts |

### Aggregates

#### IncoherenceIsolator (Aggregate Root)

```rust
use ruvector_mincut::{
    SubpolynomialMinCut, SubpolyConfig, MinCutResult, MinCutBuilder,
    CognitiveMinCutEngine, EngineConfig, WitnessTree,
    DynamicGraph, VertexId, Weight,
};

/// Isolates incoherent regions with n^o(1) updates
pub struct IncoherenceIsolator {
    /// Subpolynomial mincut
    mincut: SubpolynomialMinCut,
    /// For SNN-based continuous monitoring
    cognitive: Option<CognitiveMinCutEngine>,
}

impl IncoherenceIsolator {
    /// Build graph from high-energy edges
    pub fn from_energy(energy: &CoherenceEnergy, threshold: f32) -> Self {
        let config = SubpolyConfig::default();
        let mut mincut = SubpolynomialMinCut::new(config);

        for (edge_id, edge_energy) in &energy.edge_energies {
            if *edge_energy > threshold {
                mincut.insert_edge(
                    edge_id.source().into(),
                    edge_id.target().into(),
                    *edge_energy as f64,
                ).ok();
            }
        }

        Self { mincut, cognitive: None }
    }

    /// Find isolation cut
    pub fn isolate(&mut self) -> IsolationResult {
        let result = self.mincut.min_cut();

        IsolationResult {
            cut_value: result.value,
            partition: result.partition,
            cut_edges: result.cut_edges,
            is_exact: result.is_exact,
        }
    }

    /// Dynamic update (amortized n^o(1))
    pub fn update_edge(&mut self, source: u64, target: u64, weight: f64) -> f64 {
        self.mincut.insert_edge(source, target, weight)
            .unwrap_or(self.mincut.min_cut_value())
    }

    /// Enable SNN monitoring
    pub fn enable_cognitive(&mut self, graph: DynamicGraph) {
        self.cognitive = Some(CognitiveMinCutEngine::new(graph, EngineConfig::default()));
    }

    /// Run SNN optimization
    pub fn cognitive_optimize(&mut self, ticks: u32) -> Vec<Spike> {
        self.cognitive.as_mut()
            .map(|c| c.run(ticks))
            .unwrap_or_default()
    }
}
```

---

## Bounded Context 9: Attention-Weighted Coherence (ruvector-attention)

### Purpose

Weights residuals by structural importance using topology-gated attention, MoE routing, and PDE-based diffusion.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **TopologyGatedAttention** | Attention that considers graph structure |
| **MoE** | Mixture of Experts for specialized processing |
| **PDE Attention** | Diffusion-based energy propagation |
| **Flash Attention** | Memory-efficient attention computation |

### Aggregates

#### AttentionWeighter (Aggregate Root)

```rust
use ruvector_attention::{
    TopologyGatedAttention, TopologyGatedConfig, AttentionMode,
    MoEAttention, MoEConfig, Expert, TopKRouting,
    DiffusionAttention, DiffusionConfig, GraphLaplacian,
    FlashAttention, AttentionMask,
};

/// Attention-weighted coherence
pub struct AttentionWeighter {
    /// Topology-gated attention
    topo: TopologyGatedAttention,
    /// MoE for specialized weighting
    moe: MoEAttention,
    /// PDE diffusion
    diffusion: DiffusionAttention,
}

impl AttentionWeighter {
    /// Compute attention scores for nodes
    pub fn compute_scores(&self, states: &[&[f32]]) -> HashMap<NodeId, f32> {
        self.topo.compute_scores(states)
    }

    /// Weight residuals by attention
    pub fn weight_residuals(
        &self,
        residuals: &HashMap<EdgeId, Vec<f32>>,
        attention: &HashMap<NodeId, f32>,
    ) -> HashMap<EdgeId, f32> {
        residuals.iter()
            .map(|(edge_id, r)| {
                let src_attn = attention.get(&edge_id.source()).unwrap_or(&1.0);
                let tgt_attn = attention.get(&edge_id.target()).unwrap_or(&1.0);
                let weight = (src_attn + tgt_attn) / 2.0;

                let norm: f32 = r.iter().map(|x| x * x).sum();
                (*edge_id, norm * weight)
            })
            .collect()
    }

    /// Route through MoE
    pub fn moe_process(&self, residual: &[f32]) -> Vec<f32> {
        self.moe.forward(residual)
    }

    /// Diffuse energy across graph
    pub fn diffuse(&self, energy: &mut CoherenceEnergy, steps: usize) {
        self.diffusion.propagate(energy, steps);
    }
}
```

---

## Bounded Context 10: Distributed Consensus (ruvector-raft)

### Purpose

Synchronizes sheaf state across multiple nodes using Raft consensus for fault-tolerant distributed coherence.

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Leader** | Node responsible for log replication |
| **Follower** | Node receiving replicated entries |
| **Log Entry** | Serialized graph update |
| **Commit** | Entry replicated to majority |

### Aggregates

#### DistributedSheaf (Aggregate Root)

```rust
use ruvector_raft::{RaftNode, RaftConfig, LogEntry, ConsensusState};

/// Distributed sheaf graph with Raft consensus
pub struct DistributedSheaf {
    /// Local Raft node
    raft: RaftNode,
    /// Local graph copy
    local: SheafGraph,
}

impl DistributedSheaf {
    /// Propose update to cluster
    pub async fn propose(&mut self, update: GraphUpdate) -> Result<(), ConsensusError> {
        let entry = LogEntry::new(bincode::serialize(&update)?);
        self.raft.propose(entry).await
    }

    /// Apply committed entries
    pub fn apply_committed(&mut self) {
        while let Some(entry) = self.raft.next_committed() {
            let update: GraphUpdate = bincode::deserialize(&entry.data).unwrap();
            self.local.apply(update);
        }
    }

    /// Global energy (leader aggregates)
    pub async fn global_energy(&self) -> Result<f32, ConsensusError> {
        if self.raft.is_leader() {
            let local = self.local.compute_energy().total_energy;
            let remote: f32 = self.raft.collect_from_followers(|n| n.local_energy()).await?
                .into_iter().sum();
            Ok(local + remote)
        } else {
            self.raft.forward_to_leader(Query::GlobalEnergy).await
        }
    }
}
```

---

## Cross-Cutting Concerns

### Event Sourcing

All domain events are persisted to the event log for deterministic replay:

```rust
pub struct EventLog {
    storage: PostgresEventStore,
}

impl EventLog {
    /// Append event with signature
    pub async fn append(&self, event: DomainEvent, signer: &Signer) -> Result<SequenceId, Error> {
        let payload = bincode::serialize(&event)?;
        let signature = signer.sign(&payload);

        self.storage.insert(EventRecord {
            event_type: event.event_type(),
            payload,
            signature,
            timestamp: Timestamp::now(),
        }).await
    }

    /// Replay events from a sequence point
    pub async fn replay_from(&self, seq: SequenceId) -> impl Stream<Item = DomainEvent> {
        self.storage.stream_from(seq)
            .map(|record| bincode::deserialize(&record.payload).unwrap())
    }
}
```

### Multi-Tenancy

Isolation at data, policy, and execution boundaries:

```rust
pub struct TenantContext {
    tenant_id: TenantId,
    namespace_prefix: String,
    policy_bundle: PolicyBundleRef,
    resource_limits: ResourceLimits,
}

impl TenantContext {
    /// Scope a graph query to this tenant
    pub fn scope_query(&self, query: Query) -> Query {
        query.with_namespace_prefix(&self.namespace_prefix)
    }
}
```

### Observability

```rust
pub struct CoherenceMetrics {
    /// Energy by scope
    energy_gauge: GaugeVec,
    /// Gate decisions
    gate_decisions: CounterVec,
    /// Computation latency
    compute_latency: HistogramVec,
    /// Witness creation rate
    witness_rate: Counter,
}

impl CoherenceMetrics {
    pub fn record_energy(&self, scope: &ScopeId, energy: f32) {
        self.energy_gauge.with_label_values(&[scope.as_str()]).set(energy as f64);
    }

    pub fn record_gate_decision(&self, lane: ComputeLane, allowed: bool) {
        let labels = [lane.as_str(), if allowed { "allowed" } else { "denied" }];
        self.gate_decisions.with_label_values(&labels).inc();
    }
}
```

---

## Module Structure

```
crates/ruvector-coherence/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs                    # Public API exports
│   │
│   ├── tiles/                    # Tile Fabric (cognitum-gate-kernel)
│   │   ├── mod.rs
│   │   ├── fabric.rs             # TileFabric orchestrator
│   │   ├── adapter.rs            # CoherenceTile adapter
│   │   ├── shard.rs              # Sharding strategy
│   │   └── witness_aggregator.rs # Fragment aggregation
│   │
│   ├── sona_tuning/              # Adaptive Learning (sona)
│   │   ├── mod.rs
│   │   ├── learner.rs            # ThresholdLearner aggregate
│   │   ├── coordinator.rs        # Three-loop coordinator
│   │   └── patterns.rs           # Pattern extraction
│   │
│   ├── neural_gate/              # Neural Gating (ruvector-nervous-system)
│   │   ├── mod.rs
│   │   ├── gate.rs               # NeuralGate adapter
│   │   ├── hdc.rs                # HDC witness encoding
│   │   └── dendrite.rs           # Coincidence detection
│   │
│   ├── learned_rho/              # Learned Restriction (ruvector-gnn)
│   │   ├── mod.rs
│   │   ├── restriction.rs        # LearnedRestriction aggregate
│   │   ├── training.rs           # Training pipeline
│   │   └── ewc.rs                # EWC integration
│   │
│   ├── hyperbolic/               # Hyperbolic Coherence (ruvector-hyperbolic-hnsw)
│   │   ├── mod.rs
│   │   ├── graph.rs              # HyperbolicGraph aggregate
│   │   ├── depth.rs              # Depth computation
│   │   └── weighting.rs          # Hierarchy weighting
│   │
│   ├── mincut/                   # Incoherence Isolation (ruvector-mincut)
│   │   ├── mod.rs
│   │   ├── isolator.rs           # IncoherenceIsolator aggregate
│   │   ├── cognitive.rs          # SNN optimization
│   │   └── witness.rs            # Cut witness
│   │
│   ├── attention/                # Attention Weighting (ruvector-attention)
│   │   ├── mod.rs
│   │   ├── weighter.rs           # AttentionWeighter aggregate
│   │   ├── moe.rs                # MoE routing
│   │   └── diffusion.rs          # PDE propagation
│   │
│   ├── distributed/              # Distributed Consensus (ruvector-raft)
│   │   ├── mod.rs
│   │   ├── sheaf.rs              # DistributedSheaf aggregate
│   │   ├── replication.rs        # Log replication
│   │   └── queries.rs            # Global queries
│   │
│   ├── signal/                   # Signal Ingestion context
│   │   ├── mod.rs
│   │   ├── processor.rs          # SignalProcessor aggregate
│   │   ├── schema.rs             # EventSchema value object
│   │   └── validators.rs         # Validation chain
│   │
│   ├── substrate/                # Knowledge Substrate context
│   │   ├── mod.rs
│   │   ├── graph.rs              # SheafGraph aggregate
│   │   ├── node.rs               # SheafNode entity
│   │   ├── edge.rs               # SheafEdge entity
│   │   ├── restriction.rs        # RestrictionMap value object
│   │   └── repository.rs         # Repository trait
│   │
│   ├── coherence/                # Coherence Computation context
│   │   ├── mod.rs
│   │   ├── engine.rs             # CoherenceEngine aggregate
│   │   ├── energy.rs             # CoherenceEnergy value object
│   │   ├── spectral.rs           # SpectralAnalyzer service
│   │   └── incremental.rs        # Incremental computation
│   │
│   ├── governance/               # Governance context
│   │   ├── mod.rs
│   │   ├── policy.rs             # PolicyBundle aggregate
│   │   ├── witness.rs            # WitnessRecord entity
│   │   ├── lineage.rs            # LineageRecord entity
│   │   └── repository.rs         # Repository traits
│   │
│   ├── execution/                # Action Execution context
│   │   ├── mod.rs
│   │   ├── gate.rs               # CoherenceGate aggregate
│   │   ├── executor.rs           # ActionExecutor service
│   │   ├── action.rs             # Action trait
│   │   └── ladder.rs             # ComputeLane enum
│   │
│   ├── storage/                  # Storage infrastructure
│   │   ├── mod.rs
│   │   ├── postgres.rs           # PostgreSQL implementation
│   │   ├── ruvector.rs           # Ruvector integration
│   │   └── event_log.rs          # Event sourcing
│   │
│   ├── events.rs                 # All domain events
│   ├── error.rs                  # Domain errors
│   └── types.rs                  # Shared types (IDs, timestamps)
│
├── tests/
│   ├── integration/
│   │   ├── tiles_tests.rs        # Tile fabric tests
│   │   ├── sona_tests.rs         # Adaptive learning tests
│   │   ├── neural_tests.rs       # Neural gate tests
│   │   ├── graph_tests.rs
│   │   ├── coherence_tests.rs
│   │   └── governance_tests.rs
│   └── property/
│       ├── coherence_properties.rs
│       ├── hyperbolic_properties.rs
│       └── mincut_properties.rs
│
└── benches/
    ├── tile_bench.rs             # 256-tile throughput
    ├── sona_bench.rs             # Micro-LoRA latency
    ├── mincut_bench.rs           # Subpolynomial verification
    ├── residual_bench.rs
    └── energy_bench.rs
```

### Dependency Graph

```
ruvector-coherence
├── cognitum-gate-kernel     (tiles/)
├── sona                     (sona_tuning/)
├── ruvector-nervous-system  (neural_gate/)
├── ruvector-gnn             (learned_rho/)
├── ruvector-hyperbolic-hnsw (hyperbolic/)
├── ruvector-mincut          (mincut/)
├── ruvector-attention       (attention/)
├── ruvector-raft            (distributed/)
├── ruvector-core            (substrate/, storage/)
└── ruvector-graph           (substrate/)
```

---

## Testing Strategy

### Property-Based Tests

```rust
#[quickcheck]
fn residual_symmetry(graph: ArbitraryGraph) -> bool {
    // r_e for edge (u,v) should be negation of r_e for edge (v,u)
    // when restriction maps are transposed
    for edge in graph.edges() {
        let r_forward = edge.residual(&graph);
        let r_reverse = edge.reversed().residual(&graph);

        if !r_forward.iter().zip(r_reverse.iter())
            .all(|(a, b)| (a + b).abs() < 1e-6) {
            return false;
        }
    }
    true
}

#[quickcheck]
fn energy_non_negative(graph: ArbitraryGraph) -> bool {
    let energy = graph.compute_energy();
    energy.total_energy >= 0.0
}

#[quickcheck]
fn consistent_section_zero_energy(section: ConsistentSection) -> bool {
    // A consistent section (where all nodes agree) should have zero energy
    let graph = section.to_graph();
    let energy = graph.compute_energy();
    energy.total_energy < 1e-6
}
```

### Replay Determinism

```rust
#[test]
fn replay_produces_identical_state() {
    let events = load_event_log("test_events.log");

    // First replay
    let state1 = replay_events(&events);

    // Second replay
    let state2 = replay_events(&events);

    assert_eq!(state1.fingerprint, state2.fingerprint);
    assert_eq!(state1.energy, state2.energy);
}
```

### Chaos Testing

```rust
#[test]
fn throttling_under_chaos() {
    let gate = CoherenceGate::new(test_policy());
    let mut rng = rand::thread_rng();

    for _ in 0..10000 {
        // Random energy spikes
        let energy = if rng.gen_bool(0.1) {
            CoherenceEnergy::random_high(&mut rng)
        } else {
            CoherenceEnergy::random_normal(&mut rng)
        };

        let decision = gate.evaluate(&random_action(), &energy);

        // Verify escalation happens for high energy
        if energy.total_energy > gate.heavy_threshold() {
            assert!(decision.lane >= ComputeLane::Heavy);
        }
    }
}
```

---

## References

1. Evans, E. (2003). "Domain-Driven Design: Tackling Complexity in the Heart of Software."
2. Vernon, V. (2013). "Implementing Domain-Driven Design."
3. Hansen, J., & Ghrist, R. (2019). "Toward a spectral theory of cellular sheaves."
4. Original Architecture Gist: https://gist.github.com/ruvnet/e511e4d7015996d11ab1a1ac6d5876c0
