# ADR-014: Coherence Engine Architecture

**Status**: Proposed
**Date**: 2026-01-22
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board
**SDK**: Claude-Flow

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-01-22 | ruv.io | Initial architecture proposal |
| 0.2 | 2026-01-22 | ruv.io | Full ruvector ecosystem integration |
| 0.3 | 2026-01-22 | ruv.io | Universal coherence object, domain-agnostic interpretation, application roadmap |
| 0.4 | 2026-01-22 | ruv.io | RuvLLM integration: coherence-gated LLM inference, witness-backed generation |

---

## Context

### The Consistency Challenge

Most AI systems rely on probabilistic confidence scores to gate actions and decisions. This approach has fundamental limitations:

1. **Hallucination vulnerability** - LLMs can confidently produce incorrect outputs
2. **Drift over time** - Systems degrade without structural consistency checks
3. **Unauditable decisions** - Confidence scores don't provide provable witnesses
4. **No structural guarantees** - Probability doesn't capture logical consistency

### The Coherence Vision

> "Most systems try to get smarter by making better guesses. I am taking a different route. I want systems that stay stable under uncertainty by proving when the world still fits together and when it does not."

**This is not prediction.** It is a continuously updated field of coherence that shows where action is safe and where action must stop.

The Coherence Engine treats consistency as a **measurable first-class property** using sheaf Laplacian mathematics to compute edge-level residuals and aggregate them into coherence energy scores.

### The Universal Coherence Object

The power of this approach lies in a **single underlying coherence object** inside ruvector. Once the math is fixed, everything else becomes interpretation:

| Domain | Nodes Are | Edges Are | Residual Becomes | Gate Becomes |
|--------|-----------|-----------|------------------|--------------|
| **AI Agents** | Facts, hypotheses, beliefs | Citations, logical implication | Contradiction energy | Hallucination refusal |
| **Finance** | Trades, positions, signals | Market dependencies, arbitrage | Regime mismatch | Trading throttle |
| **Medical** | Vitals, diagnoses, treatments | Physiological causality | Clinical disagreement | Escalation trigger |
| **Robotics** | Sensor readings, goals, plans | Physics, kinematics | Motion impossibility | Safety stop |
| **Security** | Identities, permissions, actions | Policy rules, trust chains | Authorization violation | Access denial |
| **Science** | Hypotheses, observations, models | Experimental evidence | Theory inconsistency | Pruning signal |

This creates a **clean spectrum of applications without rewriting the core**.

### Why Sheaf Laplacians?

Sheaf theory provides a rigorous mathematical framework for measuring local-to-global consistency:

| Concept | Mathematical Definition | System Interpretation |
|---------|------------------------|----------------------|
| **Node** | Vertex v with state x_v | Entity with fixed-dimensional state vector (facts, trades, vitals, devices, hypotheses, beliefs) |
| **Edge** | (u, v) connection | Constraint between entities (citations, causality, physiology, policy, physics) |
| **Restriction Map** | ρ: F(U) → F(V) | How one state constrains another (lightweight linear transform) |
| **Residual** | r_e = ρ_u(x_u) - ρ_v(x_v) | **Contradiction energy** - local mismatch at edge |
| **Energy** | E(S) = Σ w_e\|r_e\|² | Global incoherence measure |
| **Gate** | E < threshold | **Refusal mechanism with witness** |

---

## The Continuously Updated Field of Coherence

The coherence engine maintains a **continuously updated field** that shows:

1. **Where action is safe** - Low energy regions where constraints are satisfied
2. **Where action must stop** - High energy regions requiring escalation or refusal

This is fundamentally different from prediction:

| Prediction-Based Systems | Coherence-Based Systems |
|--------------------------|-------------------------|
| "What will happen?" | "Does the world still fit together?" |
| Probabilistic confidence | Mathematical consistency |
| Can be confidently wrong | Knows when it doesn't know |
| Degrades silently | Alerts on structural breakdown |
| Trust the model | Trust the math |

### System Summary

The coherence engine is built on ruvector and treats consistency as a first-class, measurable property:

1. **State Modeling**: Typed graph where nodes carry fixed-dimensional vectors and edges encode constraints through lightweight restriction maps

2. **Incremental Computation**: Incoherence computed incrementally as edge-level residuals and aggregated into scoped coherence energy using a sheaf Laplacian operator

3. **Deterministic Gating**: A deterministic coherence gate controls a compute ladder. Most updates remain in a **low-latency reflex lane**, while sustained or growing incoherence triggers retrieval, deeper reasoning, or human escalation

4. **Governance by Design**: All decisions and external side effects are governed by **signed policy bundles** and produce **mandatory witness and lineage records**, making every action auditable and replayable

5. **Hybrid Storage**: PostgreSQL for transactional authority combined with ruvector for high-performance vector and graph queries

6. **Adaptive Learning**: Deterministic replay, threshold autotuning from real traces, and persistent coherence tracking allow the system to adapt without losing control

**The result is a universal inconsistency detector that scales from agent safety to autonomous systems and beyond.**

---

## Decision

### Adopt Sheaf Laplacian-Based Coherence Witnessing

We implement `ruvector-coherence` as a structural consistency engine with the following architecture:

```
+-----------------------------------------------------------------------------+
|                           APPLICATION LAYER                                  |
|  LLM Guards | Fraud Detection | Compliance Proofs | Robotics Safety         |
+-----------------------------------------------------------------------------+
                                    |
+-----------------------------------------------------------------------------+
|                           COHERENCE GATE                                     |
|  Lane 0 (Reflex) | Lane 1 (Retrieval) | Lane 2 (Heavy) | Lane 3 (Human)     |
+-----------------------------------------------------------------------------+
                                    |
+-----------------------------------------------------------------------------+
|                           COHERENCE COMPUTATION                              |
|  Residual Calculator | Energy Aggregator | Spectral Analyzer | Fingerprints |
+-----------------------------------------------------------------------------+
                                    |
+-----------------------------------------------------------------------------+
|                           GOVERNANCE LAYER                                   |
|  Policy Bundles | Witness Records | Lineage Records | Threshold Tuning      |
+-----------------------------------------------------------------------------+
                                    |
+-----------------------------------------------------------------------------+
|                           KNOWLEDGE SUBSTRATE                                |
|  Sheaf Graph | Node States | Edge Constraints | Restriction Maps           |
+-----------------------------------------------------------------------------+
                                    |
+-----------------------------------------------------------------------------+
|                           STORAGE LAYER                                      |
|  PostgreSQL (Authority) | ruvector (Graph/Vector) | Event Log (Audit)       |
+-----------------------------------------------------------------------------+
```

---

## Ruvector Ecosystem Integration

The coherence engine leverages the full ruvector crate ecosystem for maximum capability:

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                         COHERENCE ENGINE V2 ARCHITECTURE                         │
│                                                                                  │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                 COGNITUM-GATE-KERNEL (256 WASM TILES)                      │ │
│  │   Each tile: Local graph shard + E-value accumulation + Witness fragments  │ │
│  │   Memory: ~64KB/tile | Throughput: 10K+ deltas/sec | Latency: <1ms/tick    │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                       │                                          │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌───────────────┐  │
│  │ HYPERBOLIC-HNSW │ │ GNN-LEARNED     │ │ MINCUT          │ │ ATTENTION     │  │
│  │ Hierarchy-aware │ │ RESTRICTION     │ │ PARTITIONING    │ │ WEIGHTING     │  │
│  │ Poincaré energy │ │ MAPS (ρ)        │ │ n^o(1) updates  │ │ MoE/PDE/Topo  │  │
│  │ Depth scaling   │ │ EWC training    │ │ SNN integration │ │ Flash Attn    │  │
│  └─────────────────┘ └─────────────────┘ └─────────────────┘ └───────────────┘  │
│                                       │                                          │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                SONA: SELF-OPTIMIZING THRESHOLD TUNING                      │ │
│  │   Micro-LoRA (instant, <0.05ms) + Base-LoRA (background) + EWC++ (no forget)│
│  │   ReasoningBank pattern extraction | Three learning loops coordinated       │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                       │                                          │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                NERVOUS-SYSTEM: COHERENCE-GATED EXECUTION                   │ │
│  │   CoherenceGatedSystem (EXISTS!) | GlobalWorkspace | Dendritic detection   │ │
│  │   HDC witnesses (10K-dim hypervectors) | Oscillatory routing | Plasticity  │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                       │                                          │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                    RUVECTOR-RAFT: DISTRIBUTED CONSENSUS                    │ │
│  │   Multi-node sheaf synchronization | Byzantine fault tolerance             │ │
│  │   Leader election for global energy aggregation | Log replication          │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### Crate Integration Matrix

| Crate | Purpose in Coherence Engine | Key Types Used |
|-------|----------------------------|----------------|
| `cognitum-gate-kernel` | 256-tile WASM coherence fabric | `TileState`, `WitnessFragment`, `EvidenceAccumulator` |
| `sona` | Self-optimizing threshold tuning | `SonaEngine`, `MicroLoRA`, `EwcPlusPlus`, `ReasoningBank` |
| `ruvector-gnn` | Learned restriction maps | `RuvectorLayer`, `ElasticWeightConsolidation`, `ReplayBuffer` |
| `ruvector-mincut` | Subgraph isolation | `SubpolynomialMinCut`, `CognitiveMinCutEngine`, `WitnessTree` |
| `ruvector-hyperbolic-hnsw` | Hierarchy-aware energy | `HyperbolicHnsw`, `poincare_distance`, `ShardedHyperbolicHnsw` |
| `ruvector-nervous-system` | Neural gating system | `CoherenceGatedSystem`, `GlobalWorkspace`, `HdcMemory`, `Dendrite` |
| `ruvector-attention` | Attention-weighted residuals | `TopologyGatedAttention`, `MoEAttention`, `DiffusionAttention` |
| `ruvector-raft` | Distributed consensus | `RaftConsensus`, `LogReplication` |
| `ruvector-core` | Vector storage | `VectorDB`, `HnswConfig`, `DistanceMetric` |
| `ruvector-graph` | Graph operations | `GraphStore`, `AdjacencyList` |
| `ruvllm` | LLM inference with coherence | `RuvLLMEngine`, `CoherenceValidator`, `WitnessLog`, `ReasoningBank`, `AgenticMemory` |

---

## Key Components

### 1. Sheaf Graph Structure (`sheaf/`)

The mathematical foundation modeling system state as constrained graphs.

#### Node Definition

```rust
/// A node in the sheaf graph carrying a fixed-dimensional state vector
pub struct SheafNode {
    /// Unique node identifier
    pub id: NodeId,
    /// Fixed-dimensional state vector (stalks of the sheaf)
    pub state: Vec<f32>,
    /// Metadata for filtering and governance
    pub metadata: NodeMetadata,
    /// Timestamp of last state update
    pub updated_at: Timestamp,
}
```

#### Edge with Restriction Map

```rust
/// An edge encoding a constraint between two nodes
pub struct SheafEdge {
    /// Source node
    pub source: NodeId,
    /// Target node
    pub target: NodeId,
    /// Weight for energy calculation
    pub weight: f32,
    /// Restriction map from source to shared space
    pub rho_source: RestrictionMap,
    /// Restriction map from target to shared space
    pub rho_target: RestrictionMap,
}

/// Linear restriction map: Ax + b
pub struct RestrictionMap {
    /// Linear transformation matrix
    pub matrix: Matrix,
    /// Bias vector
    pub bias: Vec<f32>,
    /// Output dimension
    pub output_dim: usize,
}
```

#### Residual Calculation

```rust
impl SheafEdge {
    /// Calculate the edge residual (local mismatch)
    pub fn residual(&self, source_state: &[f32], target_state: &[f32]) -> Vec<f32> {
        let projected_source = self.rho_source.apply(source_state);
        let projected_target = self.rho_target.apply(target_state);

        // r_e = ρ_u(x_u) - ρ_v(x_v)
        projected_source.iter()
            .zip(projected_target.iter())
            .map(|(a, b)| a - b)
            .collect()
    }

    /// Calculate weighted residual norm squared
    pub fn weighted_residual_energy(&self, source: &[f32], target: &[f32]) -> f32 {
        let r = self.residual(source, target);
        let norm_sq: f32 = r.iter().map(|x| x * x).sum();
        self.weight * norm_sq
    }
}
```

### 2. Coherence Computation (`coherence/`)

Aggregates local residuals into global coherence metrics.

#### Global Energy Function

```rust
/// Global coherence energy: E(S) = Σ w_e|r_e|²
pub struct CoherenceEnergy {
    /// Total system energy (lower = more coherent)
    pub total_energy: f32,
    /// Per-edge energies for localization
    pub edge_energies: HashMap<EdgeId, f32>,
    /// Energy by scope/namespace
    pub scope_energies: HashMap<ScopeId, f32>,
    /// Computation timestamp
    pub computed_at: Timestamp,
    /// Fingerprint for change detection
    pub fingerprint: Hash,
}

impl SheafGraph {
    /// Compute global coherence energy
    pub fn compute_energy(&self) -> CoherenceEnergy {
        let edge_energies: HashMap<EdgeId, f32> = self.edges
            .par_iter()
            .map(|(id, edge)| {
                let source_state = self.nodes.get(&edge.source).unwrap().state.as_slice();
                let target_state = self.nodes.get(&edge.target).unwrap().state.as_slice();
                (*id, edge.weighted_residual_energy(source_state, target_state))
            })
            .collect();

        let total_energy: f32 = edge_energies.values().sum();

        CoherenceEnergy {
            total_energy,
            edge_energies,
            scope_energies: self.aggregate_by_scope(&edge_energies),
            computed_at: Timestamp::now(),
            fingerprint: self.compute_fingerprint(),
        }
    }
}
```

#### Incremental Computation (ADR-0002)

```rust
/// Incremental coherence update for efficiency
pub struct IncrementalCoherence {
    /// Stored per-edge residuals
    residuals: HashMap<EdgeId, Vec<f32>>,
    /// Subgraph energy summaries
    summaries: HashMap<SubgraphId, f32>,
    /// Global fingerprint for staleness detection
    global_fingerprint: Hash,
}

impl IncrementalCoherence {
    /// Update only affected edges when a node changes
    pub fn update_node(&mut self, graph: &SheafGraph, node_id: NodeId) -> CoherenceEnergy {
        // Find all edges incident to this node
        let affected_edges = graph.edges_incident_to(node_id);

        // Recompute only affected residuals
        for edge_id in affected_edges {
            let edge = graph.edges.get(&edge_id).unwrap();
            let source = graph.nodes.get(&edge.source).unwrap();
            let target = graph.nodes.get(&edge.target).unwrap();

            self.residuals.insert(edge_id, edge.residual(&source.state, &target.state));
        }

        // Update fingerprint and return
        self.recompute_energy(graph)
    }
}
```

#### Spectral Analysis

```rust
/// Spectral coherence analysis for drift detection
pub struct SpectralAnalyzer {
    /// Eigenvalue history for drift detection
    eigenvalue_history: VecDeque<Vec<f32>>,
    /// Drift threshold
    drift_threshold: f32,
}

impl SpectralAnalyzer {
    /// Detect spectral drift indicating structural change
    pub fn detect_drift(&mut self, laplacian: &SheafLaplacian) -> Option<DriftEvent> {
        let eigenvalues = laplacian.compute_eigenvalues(10); // Top 10

        if let Some(prev) = self.eigenvalue_history.back() {
            let drift = self.compute_spectral_distance(&eigenvalues, prev);

            if drift > self.drift_threshold {
                return Some(DriftEvent {
                    magnitude: drift,
                    affected_modes: self.identify_affected_modes(&eigenvalues, prev),
                    timestamp: Timestamp::now(),
                });
            }
        }

        self.eigenvalue_history.push_back(eigenvalues);
        None
    }
}
```

### 3. Coherence Gate (`gate/`)

Controls action execution based on coherence energy thresholds.

> **Key Design Principle**: Most updates remain in a **low-latency reflex lane**, while **sustained or growing** incoherence triggers retrieval, deeper reasoning, or human escalation.

#### Compute Ladder

The deterministic coherence gate sits on top of the substrate and controls a compute ladder:

```rust
/// Compute lanes for escalating complexity
///
/// CRITICAL: Most updates stay in Lane 0 (Reflex).
/// Escalation only occurs on sustained/growing incoherence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComputeLane {
    /// Lane 0: Local residual updates, simple aggregates (<1ms)
    /// THE DEFAULT - most updates stay here
    Reflex = 0,
    /// Lane 1: Evidence fetching, lightweight reasoning (~10ms)
    /// Triggered by: transient energy spike
    Retrieval = 1,
    /// Lane 2: Multi-step planning, spectral analysis (~100ms)
    /// Triggered by: sustained incoherence above threshold
    Heavy = 2,
    /// Lane 3: Human escalation for sustained incoherence
    /// Triggered by: persistent incoherence that automated systems cannot resolve
    Human = 3,
}

/// Gate evaluation result
pub struct GateDecision {
    /// Whether to allow the action
    pub allow: bool,
    /// Required compute lane
    pub lane: ComputeLane,
    /// Witness record for audit
    pub witness: WitnessRecord,
    /// Reason if denied
    pub denial_reason: Option<String>,
}
```

#### Threshold-Based Gating

```rust
/// Coherence gate with configurable thresholds
pub struct CoherenceGate {
    /// Energy threshold for Lane 0 (allow without additional checks)
    pub reflex_threshold: f32,
    /// Energy threshold for Lane 1 (require retrieval)
    pub retrieval_threshold: f32,
    /// Energy threshold for Lane 2 (require heavy compute)
    pub heavy_threshold: f32,
    /// Persistence duration before escalation
    pub persistence_window: Duration,
    /// Policy bundle reference
    pub policy_bundle: PolicyBundleRef,
}

impl CoherenceGate {
    /// Evaluate whether an action should proceed
    pub fn evaluate(
        &self,
        action: &Action,
        energy: &CoherenceEnergy,
        history: &EnergyHistory,
    ) -> GateDecision {
        let current_energy = energy.scope_energy_for(&action.scope);

        // Determine required lane based on energy
        let lane = if current_energy < self.reflex_threshold {
            ComputeLane::Reflex
        } else if current_energy < self.retrieval_threshold {
            ComputeLane::Retrieval
        } else if current_energy < self.heavy_threshold {
            ComputeLane::Heavy
        } else {
            ComputeLane::Human
        };

        // Check for persistent incoherence
        let persistent = history.is_above_threshold(
            &action.scope,
            self.retrieval_threshold,
            self.persistence_window,
        );

        // Create witness record
        let witness = WitnessRecord::new(
            action,
            energy,
            lane,
            self.policy_bundle.clone(),
        );

        // Deny if persistent incoherence and not escalated
        if persistent && lane < ComputeLane::Heavy {
            return GateDecision {
                allow: false,
                lane: ComputeLane::Heavy, // Require escalation
                witness,
                denial_reason: Some("Persistent incoherence detected".into()),
            };
        }

        GateDecision {
            allow: lane < ComputeLane::Human,
            lane,
            witness,
            denial_reason: if lane == ComputeLane::Human {
                Some("Energy exceeds all automatic thresholds".into())
            } else {
                None
            },
        }
    }
}
```

### 4. Governance Layer (`governance/`)

First-class, immutable, addressable governance objects (ADR-0005).

#### Policy Bundle

```rust
/// Versioned, signed policy bundle for threshold configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyBundle {
    /// Unique bundle identifier
    pub id: PolicyBundleId,
    /// Semantic version
    pub version: Version,
    /// Threshold configurations by scope
    pub thresholds: HashMap<ScopePattern, ThresholdConfig>,
    /// Escalation rules
    pub escalation_rules: Vec<EscalationRule>,
    /// Digital signature for integrity
    pub signature: Signature,
    /// Approvers who signed this bundle
    pub approvers: Vec<ApproverId>,
    /// Minimum required approvals
    pub required_approvals: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdConfig {
    pub reflex: f32,
    pub retrieval: f32,
    pub heavy: f32,
    pub persistence_window_secs: u64,
}
```

#### Witness Record

```rust
/// Immutable proof of every gate decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessRecord {
    /// Unique witness identifier
    pub id: WitnessId,
    /// Action that was evaluated
    pub action_hash: Hash,
    /// Energy at time of evaluation
    pub energy_snapshot: CoherenceEnergy,
    /// Gate decision made
    pub decision: GateDecision,
    /// Policy bundle used
    pub policy_bundle_ref: PolicyBundleRef,
    /// Timestamp
    pub timestamp: Timestamp,
    /// Hash chain reference to previous witness
    pub previous_witness: Option<WitnessId>,
}

impl WitnessRecord {
    /// Compute content hash for integrity
    pub fn content_hash(&self) -> Hash {
        let mut hasher = Blake3::new();
        hasher.update(&self.action_hash);
        hasher.update(&bincode::serialize(&self.energy_snapshot).unwrap());
        hasher.update(&bincode::serialize(&self.decision).unwrap());
        hasher.update(&self.policy_bundle_ref.as_bytes());
        hasher.finalize().into()
    }
}
```

#### Lineage Record

```rust
/// Provenance tracking for all authoritative writes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageRecord {
    /// Unique lineage identifier
    pub id: LineageId,
    /// Entity that was modified
    pub entity_ref: EntityRef,
    /// Operation type
    pub operation: Operation,
    /// Causal dependencies (previous lineage records)
    pub dependencies: Vec<LineageId>,
    /// Witness that authorized this write
    pub authorizing_witness: WitnessId,
    /// Actor who performed the write
    pub actor: ActorId,
    /// Timestamp
    pub timestamp: Timestamp,
}
```

### 5. Cognitum Gate Tile Fabric (`tiles/`)

Leverages the existing `cognitum-gate-kernel` for distributed coherence computation.

#### 256-Tile Architecture

```rust
use cognitum_gate_kernel::{TileState, Delta, WitnessFragment, EvidenceAccumulator};

/// Coherence fabric using 256 WASM tiles
pub struct CoherenceFabric {
    /// All tiles (each ~64KB)
    tiles: [TileState; 256],
    /// Global witness aggregator
    witness_aggregator: WitnessAggregator,
    /// Tile-to-shard mapping
    shard_map: ShardMap,
}

impl CoherenceFabric {
    /// Distribute a node update to the appropriate tile
    pub fn distribute_update(&mut self, node_id: NodeId, new_state: &[f32]) {
        let tile_id = self.shard_map.tile_for_node(node_id);
        let delta = Delta::observation(Observation::state_update(node_id, new_state));
        self.tiles[tile_id as usize].ingest_delta(&delta);
    }

    /// Execute one tick across all tiles (parallelizable)
    pub fn tick(&mut self, tick_number: u32) -> FabricReport {
        let reports: Vec<TileReport> = self.tiles
            .par_iter_mut()
            .map(|tile| tile.tick(tick_number))
            .collect();

        // Aggregate witness fragments for global coherence
        let global_witness = self.witness_aggregator.aggregate(
            reports.iter().map(|r| r.witness).collect()
        );

        // Compute global energy from tile energies
        let global_energy: f32 = reports.iter()
            .map(|r| r.log_e_value)
            .sum();

        FabricReport {
            tick: tick_number,
            global_energy,
            global_witness,
            tile_reports: reports,
        }
    }
}
```

#### E-Value Evidence Accumulation

The `cognitum-gate-kernel` already implements sequential hypothesis testing:

```rust
// From cognitum-gate-kernel - used directly
impl EvidenceAccumulator {
    /// Process observation and update e-values
    pub fn process_observation(&mut self, obs: Observation, tick: u32) {
        // E-value accumulation for anytime-valid inference
        // Allows stopping rule based on evidence strength
    }

    /// Global e-value (product of local e-values)
    pub fn global_e_value(&self) -> f64 {
        // Returns accumulated evidence for/against coherence hypothesis
    }
}
```

### 6. SONA Threshold Tuning (`sona_tuning/`)

Integrates `sona` for self-optimizing threshold management.

#### Adaptive Threshold Learning

```rust
use sona::{SonaEngine, SonaConfig, MicroLoRA, EwcPlusPlus, ReasoningBank};

/// Self-optimizing threshold tuner
pub struct SonaThresholdTuner {
    engine: SonaEngine,
    /// Pattern bank for successful threshold configurations
    reasoning_bank: ReasoningBank,
    /// Current threshold configuration
    current_thresholds: ThresholdConfig,
}

impl SonaThresholdTuner {
    pub fn new(config: SonaConfig) -> Self {
        Self {
            engine: SonaEngine::new(config),
            reasoning_bank: ReasoningBank::new(PatternConfig::default()),
            current_thresholds: ThresholdConfig::default(),
        }
    }

    /// Begin trajectory when entering a new operational regime
    pub fn begin_regime(&mut self, energy_trace: Vec<f32>) -> TrajectoryBuilder {
        self.engine.begin_trajectory(energy_trace)
    }

    /// Learn from outcome (did the thresholds work?)
    pub fn learn_outcome(&mut self, builder: TrajectoryBuilder, success_score: f32) {
        // End trajectory triggers Micro-LoRA instant learning
        self.engine.end_trajectory(builder, success_score);

        // If successful, store pattern for future reference
        if success_score > 0.8 {
            self.reasoning_bank.store_pattern(
                "threshold_success",
                &self.current_thresholds,
            );
        }
    }

    /// Query for similar past configurations
    pub fn find_similar_regime(&self, current_energy: &[f32]) -> Option<ThresholdConfig> {
        self.reasoning_bank.query_similar(current_energy, 0.9)
            .map(|pattern| pattern.decode())
    }

    /// Apply EWC++ to prevent catastrophic forgetting when learning new thresholds
    pub fn consolidate_knowledge(&mut self) {
        // EWC++ preserves important weights when adapting to new regimes
        self.engine.consolidate_ewc();
    }
}
```

#### Three Learning Loops

```rust
use sona::{InstantLoop, BackgroundLoop, LoopCoordinator};

/// Coordinated learning across three timescales
pub struct ThresholdLearningCoordinator {
    /// Instant adaptation (<0.05ms) - Micro-LoRA
    instant: InstantLoop,
    /// Background learning (async) - Base-LoRA
    background: BackgroundLoop,
    /// Coordination between loops
    coordinator: LoopCoordinator,
}

impl ThresholdLearningCoordinator {
    /// React instantly to energy spikes
    pub fn instant_adapt(&mut self, energy_spike: f32) -> ThresholdAdjustment {
        // Micro-LoRA provides immediate threshold adjustment
        self.instant.adapt(energy_spike)
    }

    /// Background optimization (runs in separate thread)
    pub fn background_optimize(&mut self, trace_history: &[EnergyTrace]) {
        self.background.optimize(trace_history);
    }

    /// Coordinate to prevent conflicts
    pub fn sync(&mut self) {
        self.coordinator.synchronize(&mut self.instant, &mut self.background);
    }
}
```

### 7. Learned Restriction Maps (`learned_rho/`)

Uses `ruvector-gnn` to learn restriction maps from data.

#### GNN-Based Restriction Map Learning

```rust
use ruvector_gnn::{
    RuvectorLayer, ElasticWeightConsolidation, ReplayBuffer,
    Optimizer, OptimizerType, LearningRateScheduler, SchedulerType,
};

/// Learned restriction map using GNN
pub struct LearnedRestrictionMap {
    /// Neural network layer for ρ
    layer: RuvectorLayer,
    /// EWC to prevent forgetting
    ewc: ElasticWeightConsolidation,
    /// Experience replay for stable learning
    replay: ReplayBuffer,
    /// Optimizer
    optimizer: Optimizer,
    /// LR scheduler
    scheduler: LearningRateScheduler,
}

impl LearnedRestrictionMap {
    pub fn new(input_dim: usize, output_dim: usize) -> Self {
        Self {
            layer: RuvectorLayer::new(input_dim, output_dim),
            ewc: ElasticWeightConsolidation::new(0.4), // λ = 0.4
            replay: ReplayBuffer::new(10000),
            optimizer: Optimizer::new(OptimizerType::Adam {
                learning_rate: 0.001,
                beta1: 0.9,
                beta2: 0.999,
                epsilon: 1e-8,
            }),
            scheduler: LearningRateScheduler::new(
                SchedulerType::CosineAnnealing { t_max: 100, eta_min: 1e-6 },
                0.001,
            ),
        }
    }

    /// Apply learned restriction map
    pub fn apply(&self, input: &[f32]) -> Vec<f32> {
        self.layer.forward(input)
    }

    /// Train on known-coherent examples
    pub fn train(&mut self, source: &[f32], target: &[f32], expected_residual: &[f32]) {
        // Store experience
        self.replay.add(source.to_vec(), target.to_vec(), expected_residual.to_vec());

        // Sample batch from replay buffer
        let batch = self.replay.sample(32);

        // Compute loss (minimize residual difference)
        let predicted = self.layer.forward_batch(&batch.sources);
        let loss = self.compute_residual_loss(&predicted, &batch.expected);

        // Backward with EWC regularization
        let ewc_loss = self.ewc.compute_ewc_loss(&self.layer);
        let total_loss = loss + ewc_loss;

        // Update
        self.optimizer.step(&mut self.layer, total_loss);
        self.scheduler.step();
    }

    /// Consolidate after training epoch (compute Fisher information)
    pub fn consolidate(&mut self) {
        self.ewc.consolidate(&self.layer);
    }
}
```

### 8. Hyperbolic Coherence (`hyperbolic/`)

Hierarchy-aware energy using `ruvector-hyperbolic-hnsw`.

#### Poincaré Ball Energy Weighting

```rust
use ruvector_hyperbolic_hnsw::{
    HyperbolicHnsw, HyperbolicHnswConfig, poincare_distance,
    project_to_ball, log_map, ShardedHyperbolicHnsw,
};

/// Hyperbolic coherence with depth-aware energy
pub struct HyperbolicCoherence {
    /// Hyperbolic index for hierarchy-aware search
    index: ShardedHyperbolicHnsw,
    /// Curvature (typically -1.0)
    curvature: f32,
}

impl HyperbolicCoherence {
    /// Compute hierarchy-weighted energy
    ///
    /// Deeper nodes (further from origin in Poincaré ball) have
    /// lower "expected" energy, so violations are weighted higher.
    pub fn weighted_energy(&self, edge: &SheafEdge, residual: &[f32]) -> f32 {
        let source_depth = self.compute_depth(&edge.source);
        let target_depth = self.compute_depth(&edge.target);
        let avg_depth = (source_depth + target_depth) / 2.0;

        // Deeper nodes: higher weight for violations (they should be more coherent)
        let depth_weight = 1.0 + avg_depth.ln().max(0.0);

        let residual_norm_sq: f32 = residual.iter().map(|x| x * x).sum();
        edge.weight * residual_norm_sq * depth_weight
    }

    /// Compute depth as distance from origin in Poincaré ball
    fn compute_depth(&self, node_id: &NodeId) -> f32 {
        let state = self.index.get_vector(node_id);
        let origin = vec![0.0; state.len()];
        poincare_distance(&state, &origin, self.curvature)
    }

    /// Project state to Poincaré ball for hierarchy-aware storage
    pub fn project_state(&self, state: &[f32]) -> Vec<f32> {
        project_to_ball(state, self.curvature)
    }
}
```

### 9. MinCut Subgraph Isolation (`mincut/`)

Uses `ruvector-mincut` for efficient incoherent region isolation.

#### Subpolynomial Dynamic MinCut

```rust
use ruvector_mincut::{
    SubpolynomialMinCut, SubpolyConfig, MinCutResult,
    CognitiveMinCutEngine, EngineConfig, WitnessTree,
};

/// Isolate incoherent subgraphs using n^o(1) mincut
pub struct IncoherenceIsolator {
    /// Subpolynomial mincut algorithm
    mincut: SubpolynomialMinCut,
    /// Cognitive engine for SNN-based optimization
    cognitive: CognitiveMinCutEngine,
}

impl IncoherenceIsolator {
    pub fn new() -> Self {
        let config = SubpolyConfig::default();
        let engine_config = EngineConfig::default();

        Self {
            mincut: SubpolynomialMinCut::new(config),
            cognitive: CognitiveMinCutEngine::new(DynamicGraph::new(), engine_config),
        }
    }

    /// Find minimum cut to isolate high-energy region
    pub fn isolate_incoherent_region(
        &mut self,
        graph: &SheafGraph,
        energy: &CoherenceEnergy,
        threshold: f32,
    ) -> IsolationResult {
        // Build weighted graph where edge weights = residual energy
        for (edge_id, edge_energy) in &energy.edge_energies {
            if *edge_energy > threshold {
                let edge = &graph.edges[edge_id];
                self.mincut.insert_edge(
                    edge.source.as_u64(),
                    edge.target.as_u64(),
                    *edge_energy as f64,
                ).ok();
            }
        }

        // Compute minimum cut (n^o(1) amortized time!)
        let result = self.mincut.min_cut();

        IsolationResult {
            cut_value: result.value,
            partition: result.partition,
            cut_edges: result.cut_edges,
        }
    }

    /// Use SNN for continuous monitoring and optimization
    pub fn cognitive_monitor(&mut self, ticks: u32) -> Vec<Spike> {
        self.cognitive.run(ticks)
    }
}
```

### 10. Neural Coherence Gate (`neural_gate/`)

Integrates `ruvector-nervous-system` for biologically-inspired gating.

#### CoherenceGatedSystem Integration

```rust
use ruvector_nervous_system::{
    CoherenceGatedSystem, GlobalWorkspace, HysteresisTracker,
    OscillatoryRouter, Dendrite, DendriticTree, HdcMemory, Hypervector,
};

/// Neural coherence gate using existing CoherenceGatedSystem
pub struct NeuralCoherenceGate {
    /// The existing coherence-gated system from ruvector-nervous-system
    system: CoherenceGatedSystem,
    /// Global workspace for conscious access
    workspace: GlobalWorkspace,
    /// Hysteresis to prevent oscillation
    hysteresis: HysteresisTracker,
    /// HDC memory for witness encoding
    hdc_memory: HdcMemory,
    /// Dendritic coincidence detection for threshold firing
    dendrites: DendriticTree,
}

impl NeuralCoherenceGate {
    /// Evaluate using biologically-inspired gating
    pub fn evaluate(&mut self, energy: f32, context: &Context) -> NeuralDecision {
        // Dendritic coincidence detection
        // Fires only if multiple "synapses" (evidence sources) are active within window
        for evidence in context.evidence_sources() {
            self.dendrites.receive_spike(evidence.id, context.timestamp);
        }

        let plateau_triggered = self.dendrites.update(context.timestamp, 1.0);

        // Hysteresis prevents rapid oscillation
        let stable_decision = self.hysteresis.filter(energy, plateau_triggered);

        // Global workspace broadcast if significant
        if stable_decision.is_significant() {
            self.workspace.broadcast(stable_decision.clone());
        }

        stable_decision
    }

    /// Encode witness as hypervector (compact, similarity-preserving)
    pub fn encode_witness(&mut self, witness: &WitnessRecord) -> Hypervector {
        // HDC encoding: bind energy + decision + policy
        let energy_hv = Hypervector::from_scalar(witness.energy_snapshot.total_energy);
        let decision_hv = Hypervector::from_enum(&witness.decision);
        let policy_hv = Hypervector::from_bytes(&witness.policy_bundle_ref.as_bytes());

        // Bind all components
        let bound = energy_hv.bind(&decision_hv).bind(&policy_hv);

        // Store in memory for similarity search
        self.hdc_memory.store(&witness.id.to_string(), bound.clone());

        bound
    }

    /// Find similar past witnesses
    pub fn find_similar_witnesses(&self, query: &Hypervector, threshold: f32) -> Vec<String> {
        self.hdc_memory.retrieve(query, threshold)
            .into_iter()
            .map(|(id, _)| id)
            .collect()
    }
}
```

### 11. Attention-Weighted Residuals (`attention/`)

Uses `ruvector-attention` for intelligent residual weighting.

#### Topology-Gated Attention

```rust
use ruvector_attention::{
    TopologyGatedAttention, TopologyGatedConfig, AttentionMode,
    MoEAttention, MoEConfig, DiffusionAttention, DiffusionConfig,
    FlashAttention,
};

/// Attention-weighted coherence computation
pub struct AttentionCoherence {
    /// Topology-gated attention (already has coherence metrics!)
    topo_attention: TopologyGatedAttention,
    /// Mixture of Experts for specialized weighting
    moe: MoEAttention,
    /// PDE-based diffusion attention for smooth propagation
    diffusion: DiffusionAttention,
}

impl AttentionCoherence {
    /// Compute attention-weighted residuals
    pub fn weighted_residuals(
        &self,
        graph: &SheafGraph,
        residuals: &HashMap<EdgeId, Vec<f32>>,
    ) -> HashMap<EdgeId, f32> {
        // Use topology-gated attention to weight by structural importance
        let node_states: Vec<&[f32]> = graph.nodes.values()
            .map(|n| n.state.as_slice())
            .collect();

        // Compute attention scores
        let attention_scores = self.topo_attention.compute_scores(&node_states);

        // Weight residuals by attention
        residuals.iter()
            .map(|(edge_id, r)| {
                let edge = &graph.edges[edge_id];
                let source_attention = attention_scores.get(&edge.source).unwrap_or(&1.0);
                let target_attention = attention_scores.get(&edge.target).unwrap_or(&1.0);

                let attention_weight = (source_attention + target_attention) / 2.0;
                let residual_norm: f32 = r.iter().map(|x| x * x).sum();

                (*edge_id, residual_norm * attention_weight)
            })
            .collect()
    }

    /// Use MoE for specialized residual processing
    pub fn moe_route_residual(&self, residual: &[f32], context: &[f32]) -> Vec<f32> {
        // Route to specialized expert based on residual characteristics
        self.moe.forward(residual, context)
    }

    /// Diffusion-based energy propagation
    pub fn diffuse_energy(&self, energy: &CoherenceEnergy, steps: usize) -> CoherenceEnergy {
        // PDE-based smoothing of energy across graph
        self.diffusion.propagate(energy, steps)
    }
}
```

### 12. Distributed Coherence (`distributed/`)

Uses `ruvector-raft` for multi-node sheaf synchronization.

#### Raft-Based Consensus

```rust
use ruvector_raft::{RaftNode, RaftConfig, LogEntry, ConsensusState};

/// Distributed coherence across multiple nodes
pub struct DistributedCoherence {
    /// Raft consensus node
    raft: RaftNode,
    /// Local sheaf graph
    local_graph: SheafGraph,
    /// Pending updates to replicate
    pending: Vec<GraphUpdate>,
}

impl DistributedCoherence {
    /// Propose a graph update to the cluster
    pub async fn propose_update(&mut self, update: GraphUpdate) -> Result<(), ConsensusError> {
        let entry = LogEntry::new(bincode::serialize(&update)?);
        self.raft.propose(entry).await?;
        Ok(())
    }

    /// Apply committed updates from Raft log
    pub fn apply_committed(&mut self) {
        while let Some(entry) = self.raft.next_committed() {
            let update: GraphUpdate = bincode::deserialize(&entry.data).unwrap();
            self.local_graph.apply_update(update);
        }
    }

    /// Get global coherence (leader aggregates from all nodes)
    pub async fn global_coherence(&self) -> Result<CoherenceEnergy, ConsensusError> {
        if self.raft.is_leader() {
            // Aggregate from all followers
            let energies = self.raft.collect_from_followers(|node| {
                node.local_coherence()
            }).await?;

            Ok(self.aggregate_energies(energies))
        } else {
            // Forward to leader
            self.raft.forward_to_leader(Request::GlobalCoherence).await
        }
    }
}
```

### 13. Storage Layer (`storage/`)

Hybrid storage with PostgreSQL for authority and ruvector for graph operations.

#### PostgreSQL Schema (Authority)

```sql
-- Policy bundles (immutable)
CREATE TABLE policy_bundles (
    id UUID PRIMARY KEY,
    version VARCHAR(32) NOT NULL,
    thresholds JSONB NOT NULL,
    escalation_rules JSONB NOT NULL,
    signature BYTEA NOT NULL,
    approvers UUID[] NOT NULL,
    required_approvals INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Witness records (append-only)
CREATE TABLE witness_records (
    id UUID PRIMARY KEY,
    action_hash BYTEA NOT NULL,
    energy_snapshot JSONB NOT NULL,
    decision JSONB NOT NULL,
    policy_bundle_id UUID REFERENCES policy_bundles(id),
    timestamp TIMESTAMPTZ NOT NULL,
    previous_witness UUID REFERENCES witness_records(id),
    content_hash BYTEA NOT NULL
);

-- Lineage records (append-only)
CREATE TABLE lineage_records (
    id UUID PRIMARY KEY,
    entity_ref JSONB NOT NULL,
    operation VARCHAR(32) NOT NULL,
    dependencies UUID[] NOT NULL,
    authorizing_witness UUID REFERENCES witness_records(id),
    actor UUID NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL
);

-- Event log (deterministic replay)
CREATE TABLE event_log (
    sequence_id BIGSERIAL PRIMARY KEY,
    event_type VARCHAR(64) NOT NULL,
    payload JSONB NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    signature BYTEA NOT NULL
);
```

#### Ruvector Integration

```rust
/// Graph substrate using ruvector for vector/graph operations
pub struct RuvectorSubstrate {
    /// Node state vectors (HNSW indexed)
    node_store: VectorDB,
    /// Edge data with restriction maps
    edge_store: GraphStore,
    /// Cached residuals for incremental computation
    residual_cache: ResidualCache,
}

impl RuvectorSubstrate {
    /// Find nodes similar to a query state
    pub async fn find_similar_nodes(
        &self,
        query_state: &[f32],
        k: usize,
    ) -> Vec<(NodeId, f32)> {
        self.node_store.search(query_state, k).await
    }

    /// Get subgraph for localized coherence computation
    pub async fn get_subgraph(&self, center: NodeId, hops: usize) -> SheafSubgraph {
        let node_ids = self.edge_store.bfs(center, hops).await;
        let nodes = self.node_store.get_batch(&node_ids).await;
        let edges = self.edge_store.edges_within(&node_ids).await;

        SheafSubgraph { nodes, edges }
    }
}
```

---

## RuvLLM Integration

Prime-Radiant integrates deeply with `ruvllm` to provide **coherence-gated LLM inference** where every generation decision is backed by structural witnesses.

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                         RUVLLM + PRIME-RADIANT INTEGRATION                       │
│                                                                                  │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                           RUVLLM ENGINE LAYER                               │ │
│  │   RuvLLMEngine | PolicyStore | SessionManager | WitnessLog | SonaIntegration │
│  └───────────────────────────────────┬────────────────────────────────────────┘ │
│                                      │                                          │
│  ┌───────────────┐ ┌───────────────┐ │ ┌───────────────┐ ┌───────────────┐     │
│  │ QUALITY       │ │ CONTEXT       │ │ │ REFLECTION    │ │ REASONING     │     │
│  │ CoherenceVal. │ │ AgenticMemory │ │ │ ReflectiveAgt │ │ ReasoningBank │     │
│  │ DiversityAna. │ │ WorkingMemory │◄─┼►│ ConfidenceChk │ │ PatternStore  │     │
│  │ QualityScore  │ │ EpisodicMem   │ │ │ ErrorLearner  │ │ EWC++ Consol. │     │
│  └───────┬───────┘ └───────┬───────┘ │ └───────┬───────┘ └───────┬───────┘     │
│          │                 │         │         │                 │              │
│          └─────────────────┼─────────┼─────────┼─────────────────┘              │
│                            ▼         │         ▼                                │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                       PRIME-RADIANT COHERENCE LAYER                         │ │
│  │                                                                              │ │
│  │   SheafGraph ◄─── Context as Nodes ◄─── Beliefs, Facts, Assertions         │ │
│  │       │                                                                      │ │
│  │   Residuals ◄─── Semantic Consistency ◄─── Citations, Implications          │ │
│  │       │                                                                      │ │
│  │   Energy ◄─── Hallucination Detector ◄─── Contradiction = High Energy       │ │
│  │       │                                                                      │ │
│  │   Gate ◄─── Inference Control ◄─── E < θ: Generate | E > θ: Refuse/Escalate │ │
│  │       │                                                                      │ │
│  │   Witness ◄─── Audit Trail ◄─── Every refusal has cryptographic proof       │ │
│  │                                                                              │ │
│  └────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### Integration Points

| RuvLLM Component | Prime-Radiant Integration | Benefit |
|------------------|---------------------------|---------|
| `CoherenceValidator` | Uses sheaf energy instead of heuristics | Mathematical consistency, not pattern matching |
| `WitnessLog` | Merged with Prime-Radiant governance | Single audit trail for all decisions |
| `ReasoningBank` | Patterns become learned restriction maps | Experience improves constraint accuracy |
| `SonaIntegration` | Shared threshold tuning | Unified adaptive learning across LLM and coherence |
| `QualityScoringEngine` | Energy-weighted quality scores | Structural quality, not just surface metrics |
| `ConfidenceChecker` | Coherence energy replaces confidence | "I don't know" is provable |
| `AgenticMemory` | Memory entries become sheaf nodes | Context consistency is computable |
| `ErrorPatternLearner` | Error patterns update restriction maps | System learns what "incoherence" means |

### Key Integration Modules

#### 1. Coherence-Backed Quality Scoring

```rust
use prime_radiant::{SheafGraph, CoherenceEnergy, CoherenceGate};
use ruvllm::quality::{CoherenceValidator, CoherenceConfig, SemanticConsistencyResult};

/// Enhanced CoherenceValidator backed by sheaf Laplacian
pub struct SheafCoherenceValidator {
    /// Prime-Radiant coherence graph
    graph: SheafGraph,
    /// Gate for inference control
    gate: CoherenceGate,
    /// Original ruvllm validator for compatibility
    inner: CoherenceValidator,
}

impl SheafCoherenceValidator {
    /// Validate response coherence using sheaf energy
    pub fn validate(&mut self, response: &str, context: &Context) -> ValidationResult {
        // 1. Convert context and response to sheaf nodes
        let context_node = self.graph.add_node(context.embedding());
        let response_node = self.graph.add_node(response.embedding());

        // 2. Add edges for semantic implications
        for claim in response.extract_claims() {
            for fact in context.facts() {
                if claim.relates_to(fact) {
                    self.graph.add_edge(
                        claim.node_id,
                        fact.node_id,
                        SemanticRestrictionMap::new(&claim, &fact)
                    );
                }
            }
        }

        // 3. Compute coherence energy
        let energy = self.graph.compute_energy();

        // 4. Gate decision with witness
        let decision = self.gate.evaluate(&Action::generate(response), &energy);

        ValidationResult {
            coherent: decision.allow,
            energy: energy.total_energy,
            witness: decision.witness,
            denial_reason: decision.denial_reason,
        }
    }
}
```

#### 2. Witness-Backed Generation

```rust
use prime_radiant::governance::{WitnessRecord, LineageRecord};
use ruvllm::{WitnessLog, WitnessEntry};

/// Unified witness log for LLM inference and coherence decisions
pub struct UnifiedWitnessLog {
    /// Prime-Radiant governance witness records
    coherence_witnesses: Vec<WitnessRecord>,
    /// RuvLLM inference witness entries
    inference_witnesses: WitnessLog,
}

impl UnifiedWitnessLog {
    /// Record generation with coherence witness
    pub fn record_generation(
        &mut self,
        prompt: &str,
        response: &str,
        coherence_decision: &GateDecision,
    ) -> GenerationWitness {
        // 1. Create Prime-Radiant witness for coherence
        let coherence_witness = coherence_decision.witness.clone();
        self.coherence_witnesses.push(coherence_witness.clone());

        // 2. Create RuvLLM witness for generation
        let inference_witness = self.inference_witnesses.record(
            WitnessEntry::generation(prompt, response)
                .with_coherence_ref(coherence_witness.id)
        );

        // 3. Create lineage linking both
        GenerationWitness {
            inference: inference_witness,
            coherence: coherence_witness,
            hash_chain: self.compute_chain_hash(),
        }
    }
}
```

#### 3. ReasoningBank → Learned Restriction Maps

```rust
use prime_radiant::learned_rho::LearnedRestrictionMap;
use ruvllm::reasoning_bank::{ReasoningBank, Pattern, Verdict};

/// Bridge ReasoningBank patterns to Prime-Radiant restriction maps
pub struct PatternToRestrictionBridge {
    /// Source patterns from RuvLLM
    reasoning_bank: ReasoningBank,
    /// Target restriction maps for Prime-Radiant
    restriction_maps: HashMap<PatternId, LearnedRestrictionMap>,
}

impl PatternToRestrictionBridge {
    /// Learn restriction map from successful patterns
    pub fn learn_from_verdict(&mut self, pattern_id: PatternId, verdict: Verdict) {
        if verdict.success_score > 0.8 {
            // Pattern succeeded - strengthen restriction map
            let pattern = self.reasoning_bank.get_pattern(pattern_id);

            // Extract source/target from pattern context
            let (source_embedding, target_embedding) = pattern.extract_embeddings();

            // Expected residual is zero for successful patterns
            let expected_residual = vec![0.0; target_embedding.len()];

            // Train restriction map to produce zero residual
            self.restriction_maps
                .entry(pattern_id)
                .or_insert_with(|| LearnedRestrictionMap::new(
                    source_embedding.len(),
                    target_embedding.len()
                ))
                .train(&source_embedding, &target_embedding, &expected_residual);
        } else {
            // Pattern failed - learn what incoherence looks like
            let pattern = self.reasoning_bank.get_pattern(pattern_id);
            let (source_embedding, target_embedding) = pattern.extract_embeddings();

            // High residual expected for failures
            let failure_residual = self.compute_failure_residual(&pattern, &verdict);

            self.restriction_maps
                .entry(pattern_id)
                .or_insert_with(|| LearnedRestrictionMap::new(
                    source_embedding.len(),
                    target_embedding.len()
                ))
                .train(&source_embedding, &target_embedding, &failure_residual);
        }
    }

    /// Export learned maps to Prime-Radiant
    pub fn export_to_prime_radiant(&self, graph: &mut SheafGraph) {
        for (pattern_id, restriction_map) in &self.restriction_maps {
            graph.register_learned_restriction(pattern_id, restriction_map.clone());
        }
    }
}
```

#### 4. Context Memory as Sheaf Nodes

```rust
use prime_radiant::substrate::SheafNode;
use ruvllm::context::{AgenticMemory, WorkingMemory, EpisodicMemory};

/// Memory entries as coherence graph nodes
pub struct MemoryCoherenceLayer {
    /// Agentic memory (long-term patterns)
    agentic: AgenticMemory,
    /// Working memory (current context)
    working: WorkingMemory,
    /// Episodic memory (conversation history)
    episodic: EpisodicMemory,
    /// Sheaf graph for coherence
    graph: SheafGraph,
}

impl MemoryCoherenceLayer {
    /// Add memory entry with coherence tracking
    pub fn add_with_coherence(&mut self, entry: MemoryEntry) -> CoherenceResult {
        // 1. Add to appropriate memory type
        let memory_id = match entry.memory_type {
            MemoryType::Agentic => self.agentic.store(entry.clone()),
            MemoryType::Working => self.working.store(entry.clone()),
            MemoryType::Episodic => self.episodic.store(entry.clone()),
        };

        // 2. Create sheaf node for memory entry
        let node = SheafNode {
            id: NodeId::from(memory_id),
            state: entry.embedding,
            metadata: entry.metadata.into(),
            updated_at: Timestamp::now(),
        };
        self.graph.add_node(node);

        // 3. Create edges to related memories
        let related = self.find_related_memories(&entry);
        for related_id in related {
            self.graph.add_edge(
                memory_id.into(),
                related_id.into(),
                MemoryRestrictionMap::temporal_consistency(),
            );
        }

        // 4. Check if adding this entry creates incoherence
        let energy = self.graph.compute_energy();

        CoherenceResult {
            memory_id,
            energy: energy.total_energy,
            coherent: energy.total_energy < self.threshold,
        }
    }
}
```

#### 5. Confidence as Coherence Energy

```rust
use prime_radiant::CoherenceEnergy;
use ruvllm::reflection::{ConfidenceChecker, ConfidenceScore};

/// Confidence derived from coherence energy
pub struct CoherenceConfidence {
    /// Base confidence checker
    inner: ConfidenceChecker,
    /// Coherence-to-confidence mapping
    energy_scale: f32,
}

impl CoherenceConfidence {
    /// Compute confidence from coherence energy
    ///
    /// Key insight: Low energy = high confidence (system is coherent)
    ///              High energy = low confidence (contradictions exist)
    pub fn confidence_from_energy(&self, energy: &CoherenceEnergy) -> ConfidenceScore {
        // Energy is non-negative, higher = more incoherent
        // Confidence should be 0-1, higher = more confident

        // Sigmoid mapping: conf = 1 / (1 + exp(scale * (energy - threshold)))
        let scaled = self.energy_scale * (energy.total_energy - self.threshold);
        let confidence = 1.0 / (1.0 + scaled.exp());

        ConfidenceScore {
            value: confidence,
            // Can explain confidence through energy breakdown
            explanation: self.explain_confidence(energy),
            // Confidence is now provable through witness
            witness_backed: true,
        }
    }

    fn explain_confidence(&self, energy: &CoherenceEnergy) -> String {
        let top_contributors: Vec<_> = energy.edge_energies
            .iter()
            .filter(|(_, e)| **e > 0.01)
            .take(3)
            .collect();

        if top_contributors.is_empty() {
            "High confidence: no structural contradictions detected".into()
        } else {
            format!(
                "Lower confidence due to {} potential inconsistencies",
                top_contributors.len()
            )
        }
    }
}
```

### Integration ADRs

| ADR | Decision |
|-----|----------|
| ADR-CE-016 | RuvLLM CoherenceValidator uses sheaf energy, not heuristic scores |
| ADR-CE-017 | WitnessLog and Prime-Radiant governance share unified audit trail |
| ADR-CE-018 | ReasoningBank patterns feed learned restriction map training |
| ADR-CE-019 | Memory entries (agentic, working, episodic) become sheaf nodes |
| ADR-CE-020 | Confidence scores derived from coherence energy with sigmoid mapping |
| ADR-CE-021 | SonaIntegration shared between ruvllm and Prime-Radiant |
| ADR-CE-022 | ErrorPatternLearner updates restriction maps on failure detection |

### Integration Benefits

1. **Structural Hallucination Detection** - Not pattern matching; mathematical proof that response contradicts context
2. **Unified Audit Trail** - Single witness chain for both inference and coherence decisions
3. **Experience-Driven Constraints** - ReasoningBank patterns make restriction maps more accurate over time
4. **Provable Confidence** - "I don't know" backed by energy calculation, not vibes
5. **Memory Consistency** - All context entries tracked for structural coherence
6. **Shared Adaptation** - SONA tunes both LLM quality and coherence thresholds together

---

## Application Tiers

> **Philosophy**: This creates a clean spectrum of applications without rewriting the core. The same residual becomes contradiction energy, and the same gate becomes a refusal mechanism with a witness.

### Tier 1: Deployable Today

| Application | Description | Coherence Use | Key Benefit |
|-------------|-------------|---------------|-------------|
| **Anti-Hallucination Guards** | Protect agents from confident incorrect outputs | Energy spike → retrieval escalation | Structural proof, not probability |
| **Market Regime Change Throttles** | Detect regime shifts before losses cascade | Spectral drift → throttle trading | Early warning, not prediction |
| **Audit-Ready Compliance Proofs** | Every decision has immutable witness trail | Witness records for every gate | Complete auditability |

### Tier 2: Next (12-24 Months)

| Application | Description | Coherence Use | Key Benefit |
|-------------|-------------|---------------|-------------|
| **Safety-First Autonomy for Drones** | Refuse action on structural mismatch | Energy threshold → motion stop | Physical safety guarantee |
| **Medical Monitoring** | Escalate only on **sustained** diagnostic disagreement | Persistence detection → alert | Reduces false positives |
| **Zero-Trust Security** | Detect structural incoherence **before** alerts fire | Graph consistency → authorization | Proactive, not reactive |

### Tier 3: Further Out (5-10 Years)

| Application | Description | Coherence Use | Key Benefit |
|-------------|-------------|---------------|-------------|
| **Scientific Discovery** | Scale discovery by **pruning inconsistent theories** | Global energy minimization | Accelerates hypothesis refinement |
| **Policy Stress Testing** | Stress-test policy futures **without pretending to predict** | Counterfactual coherence analysis | Honest uncertainty bounds |
| **Self-Awareness Primitive** | System knows **when it no longer understands itself** | Reflexive coherence monitoring | Machine metacognition |

### The Application Spectrum

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                         UNIVERSAL COHERENCE SUBSTRATE                            │
│                                                                                  │
│  ┌─────────────────────────────────────────────────────────────────────────────┐│
│  │                              SAME MATH                                       ││
│  │   Nodes: x_v (d-dimensional)    Edges: ρ_u, ρ_v    Energy: Σ w_e|r_e|²     ││
│  └─────────────────────────────────────────────────────────────────────────────┘│
│                                      │                                          │
│                                      ▼                                          │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐           │
│  │  AI AGENTS   │ │   FINANCE    │ │   MEDICAL    │ │  ROBOTICS    │           │
│  │              │ │              │ │              │ │              │           │
│  │ Beliefs →    │ │ Trades →     │ │ Vitals →     │ │ Sensors →    │           │
│  │ Citations    │ │ Arbitrage    │ │ Physiology   │ │ Physics      │           │
│  │ = Hallucin.  │ │ = Regime     │ │ = Clinical   │ │ = Motion     │           │
│  │   refusal    │ │   throttle   │ │   escalate   │ │   stop       │           │
│  └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘           │
│                                                                                  │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐                            │
│  │   SECURITY   │ │   SCIENCE    │ │ SELF-AWARE   │                            │
│  │              │ │              │ │              │                            │
│  │ Permissions →│ │ Hypotheses → │ │ Internal     │                            │
│  │ Policy       │ │ Evidence     │ │ beliefs →    │                            │
│  │ = Access     │ │ = Theory     │ │ Consistency  │                            │
│  │   denial     │ │   prune      │ │ = I don't    │                            │
│  │              │ │              │ │   know       │                            │
│  └──────────────┘ └──────────────┘ └──────────────┘                            │
│                                                                                  │
│  ┌─────────────────────────────────────────────────────────────────────────────┐│
│  │                      DIFFERENT INTERPRETATIONS                               ││
│  │       Same residual = contradiction energy | Same gate = refusal + witness  ││
│  └─────────────────────────────────────────────────────────────────────────────┘│
│                                                                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## Architectural Decision Records (Internal)

| ADR | Decision |
|-----|----------|
| ADR-CE-001 | Sheaf Laplacian defines coherence witness, not probabilistic confidence |
| ADR-CE-002 | Incremental computation with stored residuals, subgraph summaries, global fingerprints |
| ADR-CE-003 | PostgreSQL + ruvector as unified substrate |
| ADR-CE-004 | Signed event log with deterministic replay |
| ADR-CE-005 | Governance objects are first-class, immutable, addressable |
| ADR-CE-006 | Coherence gate controls explicit compute ladder (Reflex → Retrieval → Heavy → Human) |
| ADR-CE-007 | Thresholds auto-tuned from production traces with governance approval |
| ADR-CE-008 | Multi-tenant isolation at data, policy, and execution boundaries |
| ADR-CE-009 | **Single coherence object** - once math is fixed, everything is interpretation |
| ADR-CE-010 | **Domain-agnostic nodes/edges** - facts, trades, vitals, hypotheses all use same substrate |
| ADR-CE-011 | **Residual = contradiction energy** - universal interpretation across domains |
| ADR-CE-012 | **Gate = refusal mechanism with witness** - every refusal is provable |
| ADR-CE-013 | **Not prediction** - system shows safe/unsafe action, not what will happen |
| ADR-CE-014 | **Reflex lane default** - most updates stay low-latency, escalation only on sustained incoherence |
| ADR-CE-015 | **Adapt without losing control** - persistent tracking enables learning within governance |
| ADR-CE-016 | **RuvLLM CoherenceValidator** uses sheaf energy, not heuristic scores |
| ADR-CE-017 | **Unified audit trail** - WitnessLog and Prime-Radiant governance share single chain |
| ADR-CE-018 | **Pattern-to-restriction bridge** - ReasoningBank patterns feed learned restriction maps |
| ADR-CE-019 | **Memory as nodes** - AgenticMemory, WorkingMemory, EpisodicMemory become sheaf nodes |
| ADR-CE-020 | **Confidence from energy** - sigmoid mapping from coherence energy to confidence score |
| ADR-CE-021 | **Shared SONA** - SonaIntegration shared between ruvllm and Prime-Radiant |
| ADR-CE-022 | **Failure learning** - ErrorPatternLearner updates restriction maps on detection |

---

## Consequences

### Benefits

1. **Universal Inconsistency Detection** - Same math applies to agents, finance, medical, robotics, security, and science
2. **Not Prediction** - System shows where action is safe vs must stop, not what will happen
3. **Provable Consistency** - Mathematical witnesses replace probabilistic guesses
4. **Auditable Decisions** - Every gate decision has immutable witness record with lineage
5. **Localized Debugging** - Edge residuals pinpoint exact inconsistency sources
6. **Incremental Efficiency** - Only recompute affected subgraphs
7. **Low-Latency Default** - Most updates stay in reflex lane (<1ms)
8. **Graceful Escalation** - Compute ladder handles sustained/growing incoherence
9. **Governance by Design** - Signed policy bundles require multi-party approval
10. **Deterministic Replay** - Every action auditable and replayable from event log
11. **Adapt Without Losing Control** - Threshold autotuning from production traces with governance approval
12. **Domain Agnostic** - Clean spectrum of applications without rewriting core
13. **LLM Hallucination Detection** - Structural proof that response contradicts context, not pattern matching
14. **Witness-Backed Generation** - Every LLM output has cryptographic audit trail
15. **Experience-Driven Constraints** - ReasoningBank patterns improve restriction map accuracy over time
16. **Provable "I Don't Know"** - Confidence derived from energy, not heuristics

### Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Restriction map design complexity | High | Medium | Provide learned initialization from data |
| Cold start (no history) | Medium | Low | Bootstrap from domain priors |
| Computational overhead | Medium | Medium | SIMD-optimized residual calculation, incremental updates |
| Threshold tuning difficulty | Medium | Medium | Auto-tune from production traces with governance |
| Graph size scaling | Low | High | Subgraph partitioning, distributed computation |

### Performance Targets

| Metric | Target | Enabled By |
|--------|--------|------------|
| Single residual calculation | < 1us | SIMD intrinsics |
| Full graph energy (10K nodes) | < 10ms | Parallel computation |
| Incremental update (1 node) | < 100us | Tile-local updates |
| Gate evaluation | < 500us | Neural gate |
| Witness persistence | < 5ms | PostgreSQL |
| Tile tick (256 tiles parallel) | < 1ms | cognitum-gate-kernel |
| SONA instant adaptation | < 0.05ms | Micro-LoRA |
| MinCut update (amortized) | n^o(1) | Subpolynomial algorithm |
| HDC witness encoding | < 10us | Hypervector ops |
| Hyperbolic distance | < 500ns | Poincaré SIMD |
| Attention-weighted energy | < 5ms | Flash attention |
| Distributed consensus | < 50ms | Raft protocol |

---

## Implementation Phases

### Phase 1: Foundation (Weeks 1-4)

- [ ] Core sheaf graph data structures
- [ ] Residual calculation with SIMD optimization
- [ ] Basic energy aggregation
- [ ] In-memory storage backend

### Phase 2: Governance (Weeks 5-8)

- [ ] Policy bundle schema and validation
- [ ] Witness record creation and persistence
- [ ] Lineage tracking for writes
- [ ] PostgreSQL storage integration

### Phase 3: Gate (Weeks 9-12)

- [ ] Compute ladder implementation
- [ ] Threshold-based gating logic
- [ ] Persistence detection
- [ ] Escalation pathways

### Phase 4: Advanced (Weeks 13-16)

- [ ] Incremental coherence computation
- [ ] Spectral analysis for drift detection
- [ ] Auto-tuning from traces
- [ ] Multi-tenant isolation

---

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `default` | Yes | Core coherence with tiles, SONA, nervous-system |
| `full` | No | All integrations enabled |
| `tiles` | Yes | cognitum-gate-kernel 256-tile fabric |
| `sona` | Yes | Self-optimizing threshold tuning |
| `learned-rho` | Yes | GNN-learned restriction maps |
| `hyperbolic` | Yes | Hierarchy-aware Poincaré energy |
| `mincut` | Yes | Subpolynomial graph partitioning |
| `neural-gate` | Yes | Nervous-system CoherenceGatedSystem |
| `attention` | No | Attention-weighted residuals (MoE, PDE) |
| `distributed` | No | Raft-based multi-node coherence |
| `ruvllm` | No | LLM inference integration with coherence-backed generation |
| `postgres` | No | PostgreSQL governance storage |
| `simd` | Yes | SIMD-optimized residual calculation |
| `spectral` | No | Eigenvalue-based drift detection |
| `wasm` | No | WASM bindings for browser/edge |

---

## Dependencies

### Core Ruvector Crate Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `cognitum-gate-kernel` | workspace | 256-tile WASM coherence fabric |
| `sona` | workspace | Self-optimizing thresholds with EWC++ |
| `ruvector-gnn` | workspace | Learned restriction maps, replay buffers |
| `ruvector-mincut` | workspace | Subpolynomial n^o(1) graph partitioning |
| `ruvector-hyperbolic-hnsw` | workspace | Hierarchy-aware Poincaré energy |
| `ruvector-nervous-system` | workspace | CoherenceGatedSystem, HDC witnesses |
| `ruvector-attention` | workspace | Topology-gated attention, MoE |
| `ruvector-raft` | workspace | Distributed consensus |
| `ruvector-core` | workspace | Vector storage and HNSW search |
| `ruvector-graph` | workspace | Graph data structures |
| `ruvllm` | workspace | LLM inference with coherence-backed quality |

### External Dependencies

| Dependency | Purpose |
|------------|---------|
| `ndarray` | Matrix operations for restriction maps |
| `rayon` | Parallel residual computation |
| `blake3` | Content hashing for witnesses |
| `bincode` | Binary serialization |
| `tokio` | Async runtime for distributed coherence |

### Optional Dependencies

| Dependency | Feature | Purpose |
|------------|---------|---------|
| `sqlx` | postgres | PostgreSQL async client |
| `nalgebra` | spectral | Eigenvalue computation |
| `serde_json` | - | JSON serialization for governance |
| `wasm-bindgen` | wasm | WASM bindings for browser deployment |

---

## References

1. Hansen, J., & Ghrist, R. (2019). "Toward a spectral theory of cellular sheaves." Journal of Applied and Computational Topology.

2. Curry, J. (2014). "Sheaves, Cosheaves and Applications." arXiv:1303.3255.

3. Robinson, M. (2014). "Topological Signal Processing." Springer.

4. RuVector Team. "ruvector-core Architecture." ADR-001.

5. Original Gist: "Coherence Engine Vision." https://gist.github.com/ruvnet/e511e4d7015996d11ab1a1ac6d5876c0

---

## Related Decisions

- **ADR-001**: Ruvector Core Architecture
- **ADR-003**: SIMD Optimization Strategy
- **ADR-006**: Memory Management
- **ADR-007**: Security Review & Technical Debt
- **ADR-011**: RuvLLM Architecture (LLM serving with quality gates)
- **ADR-012**: ReasoningBank Pattern Storage (EWC++ consolidation)
