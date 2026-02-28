# DDD-001: Coherence Gate Domain Model

**Status**: Proposed
**Date**: 2026-01-17
**Authors**: ruv.io, RuVector Team
**Related ADR**: ADR-001-ruqu-architecture

---

## Overview

This document defines the Domain-Driven Design model for the Coherence Gate—the core decision-making subsystem that determines whether a quantum system region is coherent enough to trust action.

---

## Strategic Design

### Domain Vision Statement

> The Coherence Gate domain provides real-time, microsecond-scale structural awareness of quantum system health, enabling adaptive control decisions that were previously impossible with static policies.

### Core Domain

**Coherence Assessment** is the core domain. This is what differentiates ruQu from all other quantum control approaches:

- Not decoding (that's a supporting domain)
- Not syndrome collection (that's infrastructure)
- **The novel capability**: Answering "Is this region still internally consistent enough to trust action?"

### Supporting Domains

| Domain | Role | Boundary |
|--------|------|----------|
| **Syndrome Ingestion** | Collect and buffer syndrome data | Generic, infrastructure |
| **Graph Maintenance** | Keep operational graph current | Generic, infrastructure |
| **Cryptographic Receipts** | Audit trail and permits | Generic, security |
| **Decoder Integration** | Apply corrections | External, existing |

### Generic Subdomains

- Logging and observability
- Configuration management
- Communication protocols

---

## Ubiquitous Language

### Core Terms

| Term | Definition | Context |
|------|------------|---------|
| **Coherence** | The property of a quantum system region being internally consistent and operationally trustworthy | Domain core |
| **Gate Decision** | The output of coherence assessment: PERMIT, DEFER, or DENY | Domain core |
| **Permit Token** | A signed capability authorizing action on a coherent region | Domain core |
| **Witness** | Cryptographic proof of the graph state at decision time | Domain core |
| **Quarantine** | Isolation of an incoherent region from action | Domain core |

### Structural Terms

| Term | Definition | Context |
|------|------------|---------|
| **Operational Graph** | A weighted graph capturing all elements affecting coherence | Model |
| **Min-Cut** | The minimum weight of edges separating healthy from unhealthy partitions | Algorithm |
| **Cut Value** | Numeric measure of structural fragility—low value means boundary forming | Metric |
| **Boundary** | The set of edges in the min-cut, identifying the fracture point | Diagnostic |

### Statistical Terms

| Term | Definition | Context |
|------|------------|---------|
| **Shift** | Aggregate nonconformity indicating distribution drift | Filter 2 |
| **E-Value** | Running evidence accumulator for anytime-valid testing | Filter 3 |
| **Threshold** | Decision boundary for each filter | Configuration |

### Architectural Terms

| Term | Definition | Context |
|------|------------|---------|
| **Tile** | A processing unit handling a graph shard | Architecture |
| **TileZero** | The coordinator tile that merges reports and makes global decisions | Architecture |
| **Worker Tile** | One of 255 tiles processing local graph shards | Architecture |
| **Fabric** | The full 256-tile processing array | Architecture |

---

## Bounded Contexts

### Context Map

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           COHERENCE GATE CONTEXT                            │
│                              (Core Domain)                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │  Decision   │  │   Filter    │  │   Graph     │  │   Permit    │        │
│  │   Engine    │  │  Pipeline   │  │   Model     │  │   Manager   │        │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────────────────────┘
          │                 │                 │                 │
          │ Upstream        │ Upstream        │ Upstream        │ Downstream
          ▼                 ▼                 ▼                 ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│    SYNDROME     │ │   CALIBRATION   │ │    HARDWARE     │ │    DECODER      │
│    CONTEXT      │ │    CONTEXT      │ │    CONTEXT      │ │    CONTEXT      │
│  (Supporting)   │ │  (Supporting)   │ │   (External)    │ │   (External)    │
└─────────────────┘ └─────────────────┘ └─────────────────┘ └─────────────────┘
```

### Coherence Gate Context (Core)

**Responsibility**: Make coherence decisions and issue permits

**Key Aggregates**:
- GateDecision
- PermitToken
- CoherenceState

**Anti-Corruption Layers**:
- Syndrome Adapter (translates raw syndromes to events)
- Hardware Adapter (translates hardware state to graph updates)
- Decoder Adapter (translates decisions to decoder commands)

### Syndrome Context (Supporting)

**Responsibility**: Collect, buffer, and deliver syndrome streams

**Key Aggregates**:
- SyndromeRound
- SyndromeBuffer
- DetectorMap

**Relationship**: Conforms to Coherence Gate Context

### Calibration Context (Supporting)

**Responsibility**: Manage calibration state and trigger recalibration

**Key Aggregates**:
- CalibrationSnapshot
- DriftIndicator
- CalibrationTrigger

**Relationship**: Customer-Supplier with Coherence Gate Context

---

## Aggregates

### GateDecision (Root Aggregate)

The central aggregate representing a coherence assessment outcome.

```
┌─────────────────────────────────────────────────────────────────┐
│                      GATE DECISION                              │
│                    (Aggregate Root)                             │
├─────────────────────────────────────────────────────────────────┤
│  decision_id: DecisionId                                        │
│  timestamp: Timestamp                                           │
│  verdict: Verdict { Permit | Defer | Deny }                     │
│  region_mask: RegionMask                                        │
│  filter_results: FilterResults                                  │
│  witness: Option<Witness>                                       │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ FilterResults (Value Object)                            │   │
│  │  structural: StructuralResult { cut_value, boundary }   │   │
│  │  shift: ShiftResult { pressure, affected_regions }      │   │
│  │  evidence: EvidenceResult { e_value, confidence }       │   │
│  └─────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────┤
│  Invariants:                                                    │
│  - All three filters must be evaluated                          │
│  - PERMIT requires all filters pass                             │
│  - DENY requires at least one filter hard-fail                  │
│  - Witness required for DENY decisions                          │
└─────────────────────────────────────────────────────────────────┘
```

### PermitToken (Aggregate)

A signed capability authorizing action.

```
┌─────────────────────────────────────────────────────────────────┐
│                      PERMIT TOKEN                               │
│                    (Aggregate Root)                             │
├─────────────────────────────────────────────────────────────────┤
│  token_id: TokenId                                              │
│  decision_id: DecisionId                                        │
│  action_id: ActionId                                            │
│  region_mask: RegionMask                                        │
│  issued_at: Timestamp                                           │
│  expires_at: Timestamp                                          │
│  signature: Ed25519Signature                                    │
│  witness_hash: Blake3Hash                                       │
├─────────────────────────────────────────────────────────────────┤
│  Invariants:                                                    │
│  - Signature must be valid Ed25519 (64 bytes)                   │
│  - expires_at > issued_at                                       │
│  - TTL bounded by configuration                                 │
│  - witness_hash matches decision witness                        │
└─────────────────────────────────────────────────────────────────┘
```

### OperationalGraph (Aggregate)

The graph model of system coherence.

```
┌─────────────────────────────────────────────────────────────────┐
│                   OPERATIONAL GRAPH                             │
│                    (Aggregate Root)                             │
├─────────────────────────────────────────────────────────────────┤
│  graph_id: GraphId                                              │
│  version: Version (monotonic)                                   │
│  vertices: Map<VertexId, Vertex>                                │
│  edges: Map<EdgeId, Edge>                                       │
│  partitions: Map<PartitionId, Partition>                        │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Vertex (Entity)                                         │   │
│  │  vertex_id: VertexId                                    │   │
│  │  vertex_type: VertexType { Qubit | Coupler | ... }      │   │
│  │  health_state: HealthState { Healthy | Degraded | ... } │   │
│  │  metadata: VertexMetadata                               │   │
│  └─────────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Edge (Entity)                                           │   │
│  │  edge_id: EdgeId                                        │   │
│  │  source: VertexId                                       │   │
│  │  target: VertexId                                       │   │
│  │  weight: EdgeWeight (coherence coupling strength)       │   │
│  │  edge_type: EdgeType { Coupling | Crosstalk | ... }     │   │
│  └─────────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────────┤
│  Invariants:                                                    │
│  - Version only increases                                       │
│  - No orphan vertices (all must be reachable)                   │
│  - Edge weights non-negative                                    │
│  - Partition assignment complete (every vertex in one partition)│
└─────────────────────────────────────────────────────────────────┘
```

---

## Value Objects

### RegionMask

Identifies which regions are affected by a decision.

```rust
struct RegionMask {
    bits: u256,  // One bit per tile (256 tiles)
}

impl RegionMask {
    fn all() -> Self;
    fn none() -> Self;
    fn from_tiles(tiles: &[TileId]) -> Self;
    fn intersects(&self, other: &RegionMask) -> bool;
    fn union(&self, other: &RegionMask) -> RegionMask;
}
```

### Verdict

The three-valued decision outcome.

```rust
enum Verdict {
    Permit,  // Action authorized
    Defer,   // Needs human review
    Deny,    // Action blocked
}
```

### CutValue

The min-cut metric with its interpretation.

```rust
struct CutValue {
    value: f64,
    threshold: f64,
    boundary_edges: Vec<EdgeId>,
}

impl CutValue {
    fn is_coherent(&self) -> bool {
        self.value >= self.threshold
    }

    fn fragility(&self) -> f64 {
        self.threshold / self.value.max(0.001)
    }
}
```

### EvidenceAccumulator

Running e-value with anytime-valid properties.

```rust
struct EvidenceAccumulator {
    log_e_value: f64,
    samples_seen: u64,
    wealth_sequence: VecDeque<f64>,
}

impl EvidenceAccumulator {
    fn update(&mut self, score: f64);
    fn current_e(&self) -> f64;
    fn verdict(&self, tau_permit: f64, tau_deny: f64) -> Option<Verdict>;
}
```

---

## Domain Events

### Core Events

| Event | Trigger | Payload |
|-------|---------|---------|
| `CoherenceAssessed` | Every cycle | decision_id, verdict, filter_results |
| `PermitIssued` | PERMIT decision | token, action_id, region_mask |
| `QuarantineInitiated` | DENY decision | region_mask, witness, recovery_mode |
| `DeferEscalated` | DEFER decision | decision_id, reason, suggested_reviewer |

### Graph Events

| Event | Trigger | Payload |
|-------|---------|---------|
| `GraphUpdated` | Syndrome arrival | version, delta |
| `VertexDegraded` | Health change | vertex_id, old_state, new_state |
| `EdgeWeightChanged` | Coupling drift | edge_id, old_weight, new_weight |
| `PartitionSplit` | Cut detected | old_partition, new_partitions |

### Filter Events

| Event | Trigger | Payload |
|-------|---------|---------|
| `StructuralBoundaryForming` | Cut dropping | cut_value, boundary_edges, trend |
| `ShiftPressureRising` | Drift detected | shift_value, affected_regions |
| `EvidenceThresholdCrossed` | E-value crosses τ | e_value, direction, decision |

---

## Domain Services

### CoherenceGateService

The orchestrating service that runs the three-filter pipeline.

```rust
trait CoherenceGateService {
    /// Evaluate coherence for the current cycle
    async fn evaluate(&self, cycle: CycleId) -> GateDecision;

    /// Issue a permit token for an action
    async fn issue_permit(&self, action: ActionContext) -> Result<PermitToken, GateError>;

    /// Verify a permit token
    fn verify_permit(&self, token: &PermitToken) -> Result<(), VerifyError>;

    /// Get current coherence state
    fn current_state(&self) -> CoherenceState;
}
```

### FilterPipelineService

Runs the three stacked filters.

```rust
trait FilterPipelineService {
    /// Run structural filter (min-cut)
    fn evaluate_structural(&self, graph: &OperationalGraph) -> StructuralResult;

    /// Run shift filter (conformal)
    fn evaluate_shift(&self, syndromes: &SyndromeBuffer) -> ShiftResult;

    /// Run evidence filter (e-value)
    fn evaluate_evidence(&self, accumulator: &EvidenceAccumulator) -> EvidenceResult;

    /// Combine filter results into verdict
    fn combine(&self, structural: StructuralResult, shift: ShiftResult, evidence: EvidenceResult) -> Verdict;
}
```

### WitnessService

Generates cryptographic witnesses for decisions.

```rust
trait WitnessService {
    /// Generate witness for current graph state
    fn generate(&self, graph: &OperationalGraph, decision: &GateDecision) -> Witness;

    /// Verify witness against historical state
    fn verify(&self, witness: &Witness, receipt_chain: &ReceiptChain) -> Result<(), WitnessError>;
}
```

---

## Repositories

### GateDecisionRepository

```rust
trait GateDecisionRepository {
    async fn store(&self, decision: GateDecision) -> Result<(), StoreError>;
    async fn find_by_id(&self, id: DecisionId) -> Option<GateDecision>;
    async fn find_by_cycle(&self, cycle: CycleId) -> Option<GateDecision>;
    async fn find_in_range(&self, start: CycleId, end: CycleId) -> Vec<GateDecision>;
}
```

### PermitTokenRepository

```rust
trait PermitTokenRepository {
    async fn store(&self, token: PermitToken) -> Result<(), StoreError>;
    async fn find_by_id(&self, id: TokenId) -> Option<PermitToken>;
    async fn find_active(&self) -> Vec<PermitToken>;
    async fn revoke(&self, id: TokenId) -> Result<(), RevokeError>;
}
```

### OperationalGraphRepository

```rust
trait OperationalGraphRepository {
    async fn current(&self) -> OperationalGraph;
    async fn at_version(&self, version: Version) -> Option<OperationalGraph>;
    async fn apply_delta(&self, delta: GraphDelta) -> Result<Version, ApplyError>;
}
```

---

## Factories

### GateDecisionFactory

```rust
impl GateDecisionFactory {
    fn create_permit(
        filter_results: FilterResults,
        region_mask: RegionMask,
    ) -> GateDecision {
        GateDecision {
            decision_id: DecisionId::new(),
            timestamp: Timestamp::now(),
            verdict: Verdict::Permit,
            region_mask,
            filter_results,
            witness: None,
        }
    }

    fn create_deny(
        filter_results: FilterResults,
        region_mask: RegionMask,
        boundary: Vec<EdgeId>,
    ) -> GateDecision {
        let witness = WitnessService::generate_for_boundary(&boundary);
        GateDecision {
            decision_id: DecisionId::new(),
            timestamp: Timestamp::now(),
            verdict: Verdict::Deny,
            region_mask,
            filter_results,
            witness: Some(witness),
        }
    }
}
```

---

## Invariants and Business Rules

### Decision Invariants

1. **Three-Filter Agreement**: PERMIT requires all three filters to pass
2. **Witness on Deny**: Every DENY decision must have a witness
3. **Monotonic Sequence**: Decision sequence numbers only increase
4. **Bounded Latency**: Decision must complete within 4μs budget

### Token Invariants

1. **Valid Signature**: Token signature must verify with TileZero public key
2. **Temporal Validity**: Token only valid between issued_at and expires_at
3. **Region Consistency**: Token region_mask must match decision region_mask
4. **Single Use**: Token action_id must be unique (no replay)

### Graph Invariants

1. **Version Monotonicity**: Graph version only increases
2. **Edge Consistency**: Edges reference valid vertices
3. **Partition Completeness**: Every vertex belongs to exactly one partition
4. **Weight Non-Negativity**: All edge weights ≥ 0

---

## Anti-Corruption Layers

### Syndrome ACL

Translates raw hardware syndromes to domain events.

```rust
impl SyndromeAntiCorruptionLayer {
    fn translate(&self, raw: RawSyndromePacket) -> SyndromeEvent {
        SyndromeEvent {
            round: self.extract_round(raw),
            detectors: self.decode_detectors(raw),
            timestamp: self.normalize_timestamp(raw),
        }
    }
}
```

### Decoder ACL

Translates gate decisions to decoder commands.

```rust
impl DecoderAntiCorruptionLayer {
    fn translate(&self, decision: &GateDecision) -> DecoderCommand {
        match decision.verdict {
            Verdict::Permit => DecoderCommand::NormalMode,
            Verdict::Defer => DecoderCommand::ConservativeMode,
            Verdict::Deny => DecoderCommand::Pause(decision.region_mask),
        }
    }
}
```

---

## Context Boundaries Summary

| Boundary | Upstream | Downstream | Integration Pattern |
|----------|----------|------------|---------------------|
| Syndrome → Gate | Syndrome Context | Gate Context | Published Language (SyndromeEvent) |
| Gate → Decoder | Gate Context | Decoder Context | ACL (DecoderCommand) |
| Gate → Calibration | Gate Context | Calibration Context | Domain Events (DriftDetected) |
| Hardware → Gate | Hardware Context | Gate Context | ACL (GraphDelta) |

---

## References

- ADR-001: ruQu Architecture
- Evans, Eric. "Domain-Driven Design." Addison-Wesley, 2003.
- Vernon, Vaughn. "Implementing Domain-Driven Design." Addison-Wesley, 2013.
