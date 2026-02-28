# ADR-DB-001: Delta Behavior Core Architecture

**Status**: Proposed
**Date**: 2026-01-28
**Authors**: RuVector Architecture Team
**Deciders**: Architecture Review Board
**SDK**: Claude-Flow

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-01-28 | Architecture Team | Initial proposal |

---

## Context and Problem Statement

### The Incremental Update Challenge

Traditional vector databases treat updates as atomic replacements - when a vector changes, the entire vector is stored and the index is rebuilt or patched. This approach has significant limitations:

1. **Network Inefficiency**: Transmitting full vectors for minor adjustments wastes bandwidth
2. **Storage Bloat**: Write-ahead logs grow linearly with vector dimensions
3. **Index Thrashing**: Frequent small changes cause excessive index reorganization
4. **Temporal Blindness**: Update history is lost, preventing rollback and analysis
5. **Concurrency Bottlenecks**: Full vector locks block concurrent partial updates

### Current Ruvector State

Ruvector's existing architecture (ADR-001) uses:
- Full vector replacement via `VectorEntry` structs
- HNSW index with mark-delete (no true incremental update)
- REDB transactions at vector granularity
- No delta compression or tracking

### The Delta-First Vision

Delta-Behavior transforms ruvector into a **delta-first vector database** where:
- All mutations are expressed as deltas (incremental changes)
- Full vectors are composed from delta chains on read
- Indexes support incremental updates with quality guarantees
- Conflict resolution uses CRDT semantics for concurrent edits

---

## Decision

### Adopt Delta-First Architecture with Layered Composition

We implement a delta-first architecture with the following design principles:

```
+-----------------------------------------------------------------------------+
|                         DELTA APPLICATION LAYER                              |
|  Delta API | Vector Composition | Temporal Queries | Rollback               |
+-----------------------------------------------------------------------------+
                                   |
+-----------------------------------------------------------------------------+
|                         DELTA PROPAGATION LAYER                              |
|  Reactive Push | Backpressure | Causal Ordering | Broadcast                 |
+-----------------------------------------------------------------------------+
                                   |
+-----------------------------------------------------------------------------+
|                         DELTA CONFLICT LAYER                                 |
|  CRDT Merge | Vector Clocks | Operational Transform | Conflict Detection   |
+-----------------------------------------------------------------------------+
                                   |
+-----------------------------------------------------------------------------+
|                         DELTA INDEX LAYER                                    |
|  Lazy Repair | Quality Bounds | Checkpoint Snapshots | Incremental HNSW    |
+-----------------------------------------------------------------------------+
                                   |
+-----------------------------------------------------------------------------+
|                         DELTA ENCODING LAYER                                 |
|  Sparse | Dense | Run-Length | Dictionary | Adaptive Switching             |
+-----------------------------------------------------------------------------+
                                   |
+-----------------------------------------------------------------------------+
|                         DELTA STORAGE LAYER                                  |
|  Append-Only Log | Delta Chains | Compaction | Compression                  |
+-----------------------------------------------------------------------------+
```

### Core Data Structures

#### Delta Representation

```rust
/// A delta representing an incremental change to a vector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorDelta {
    /// Unique delta identifier
    pub delta_id: DeltaId,
    /// Target vector this delta applies to
    pub vector_id: VectorId,
    /// Parent delta (for causal ordering)
    pub parent_delta: Option<DeltaId>,
    /// The actual change
    pub operation: DeltaOperation,
    /// Vector clock for conflict detection
    pub clock: VectorClock,
    /// Timestamp of creation
    pub timestamp: DateTime<Utc>,
    /// Replica that created this delta
    pub origin_replica: ReplicaId,
    /// Optional metadata changes
    pub metadata_delta: Option<MetadataDelta>,
}

/// Types of delta operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeltaOperation {
    /// Create a new vector (full vector as delta from zero)
    Create { vector: Vec<f32> },
    /// Sparse update: change specific dimensions
    Sparse { indices: Vec<u32>, values: Vec<f32> },
    /// Dense update: full vector replacement
    Dense { vector: Vec<f32> },
    /// Scale all dimensions
    Scale { factor: f32 },
    /// Add offset to all dimensions
    Offset { amount: f32 },
    /// Apply element-wise transformation
    Transform { transform_id: TransformId },
    /// Delete the vector
    Delete,
}
```

#### Delta Chain

```rust
/// A chain of deltas composing a vector's history
pub struct DeltaChain {
    /// Vector ID this chain represents
    pub vector_id: VectorId,
    /// Checkpoint: materialized snapshot
    pub checkpoint: Option<Checkpoint>,
    /// Deltas since last checkpoint
    pub pending_deltas: Vec<VectorDelta>,
    /// Current materialized vector (cached)
    pub current: Option<Vec<f32>>,
    /// Chain metadata
    pub metadata: ChainMetadata,
}

/// Materialized snapshot for efficient composition
pub struct Checkpoint {
    pub vector: Vec<f32>,
    pub at_delta: DeltaId,
    pub timestamp: DateTime<Utc>,
    pub delta_count: u64,
}
```

### Delta Lifecycle

```
                    ┌─────────────────────────────────────────────────────┐
                    │                   DELTA LIFECYCLE                    │
                    └─────────────────────────────────────────────────────┘

    ┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐
    │ CREATE  │ --> │ ENCODE  │ --> │PROPAGATE│ --> │ RESOLVE │ --> │  APPLY  │
    └─────────┘     └─────────┘     └─────────┘     └─────────┘     └─────────┘
         │               │               │               │               │
         v               v               v               v               v
    ┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐
    │ Delta   │     │ Hybrid  │     │Reactive │     │  CRDT   │     │  Lazy   │
    │Operation│     │Encoding │     │  Push   │     │  Merge  │     │ Repair  │
    └─────────┘     └─────────┘     └─────────┘     └─────────┘     └─────────┘
```

---

## Decision Drivers

### 1. Network Efficiency (Minimize Bandwidth)

| Requirement | Implementation |
|-------------|----------------|
| Sparse updates | Only transmit changed dimensions |
| Delta compression | Multi-tier encoding strategies |
| Batching | Temporal windows for aggregation |

### 2. Storage Efficiency (Minimize Writes)

| Requirement | Implementation |
|-------------|----------------|
| Append-only log | Delta log with periodic compaction |
| Checkpointing | Materialized snapshots at intervals |
| Compression | LZ4/Zstd on delta batches |

### 3. Consistency (Strong Guarantees)

| Requirement | Implementation |
|-------------|----------------|
| Causal ordering | Vector clocks per delta |
| Conflict resolution | CRDT-based merge semantics |
| Durability | WAL with delta granularity |

### 4. Performance (Low Latency)

| Requirement | Implementation |
|-------------|----------------|
| Read path | Cached current vectors |
| Write path | Async delta propagation |
| Index updates | Lazy repair with quality bounds |

---

## Considered Options

### Option 1: Full Vector Replacement (Status Quo)

**Description**: Continue with atomic vector replacement.

**Pros**:
- Simple implementation
- No composition overhead on reads
- Index always exact

**Cons**:
- Network inefficient for sparse updates
- No temporal history
- No concurrent partial updates

**Verdict**: Rejected - does not meet incremental update requirements.

### Option 2: Event Sourcing with Vector Events

**Description**: Full event sourcing where current state is derived from event log.

**Pros**:
- Complete audit trail
- Perfect temporal queries
- Natural undo/redo

**Cons**:
- Read amplification (must replay all events)
- Unbounded storage growth
- Complex query semantics

**Verdict**: Partially adopted - delta log is event-sourced with materialization.

### Option 3: Delta-First with Materialized Views

**Description**: Primary storage is deltas; materialized vectors are caches.

**Pros**:
- Best of both worlds
- Efficient writes (delta only)
- Efficient reads (materialized cache)
- Full temporal history

**Cons**:
- Cache invalidation complexity
- Checkpoint management
- Conflict resolution needed

**Verdict**: Adopted - provides optimal balance.

### Option 4: Operational Transformation (OT)

**Description**: Use OT for concurrent delta resolution.

**Pros**:
- Well-understood concurrency model
- Used by Google Docs, etc.

**Cons**:
- Complex transformation functions
- Central server typically required
- Vector semantics don't map cleanly

**Verdict**: Rejected - CRDT better suited for vector semantics.

---

## Technical Specification

### Delta API

```rust
/// Delta-aware vector database trait
pub trait DeltaVectorDB: Send + Sync {
    /// Apply a delta to a vector
    fn apply_delta(&self, delta: VectorDelta) -> Result<DeltaId>;

    /// Apply multiple deltas atomically
    fn apply_deltas(&self, deltas: Vec<VectorDelta>) -> Result<Vec<DeltaId>>;

    /// Get current vector (composing from delta chain)
    fn get_vector(&self, id: &VectorId) -> Result<Option<Vec<f32>>>;

    /// Get vector at specific point in time
    fn get_vector_at(&self, id: &VectorId, timestamp: DateTime<Utc>)
        -> Result<Option<Vec<f32>>>;

    /// Get delta chain for a vector
    fn get_delta_chain(&self, id: &VectorId) -> Result<DeltaChain>;

    /// Rollback to specific delta
    fn rollback_to(&self, id: &VectorId, delta_id: &DeltaId) -> Result<()>;

    /// Compact delta chain (merge deltas, create checkpoint)
    fn compact(&self, id: &VectorId) -> Result<()>;

    /// Search with delta-aware semantics
    fn search_delta(&self, query: &DeltaSearchQuery) -> Result<Vec<SearchResult>>;
}
```

### Composition Algorithm

```rust
impl DeltaChain {
    /// Compose current vector from checkpoint and pending deltas
    pub fn compose(&self) -> Result<Vec<f32>> {
        // Start from checkpoint or zero vector
        let mut vector = match &self.checkpoint {
            Some(cp) => cp.vector.clone(),
            None => vec![0.0; self.dimensions],
        };

        // Apply pending deltas in causal order
        for delta in self.pending_deltas.iter() {
            self.apply_operation(&mut vector, &delta.operation)?;
        }

        Ok(vector)
    }

    fn apply_operation(&self, vector: &mut Vec<f32>, op: &DeltaOperation) -> Result<()> {
        match op {
            DeltaOperation::Sparse { indices, values } => {
                for (idx, val) in indices.iter().zip(values.iter()) {
                    if (*idx as usize) < vector.len() {
                        vector[*idx as usize] = *val;
                    }
                }
            }
            DeltaOperation::Dense { vector: new_vec } => {
                vector.copy_from_slice(new_vec);
            }
            DeltaOperation::Scale { factor } => {
                for v in vector.iter_mut() {
                    *v *= factor;
                }
            }
            DeltaOperation::Offset { amount } => {
                for v in vector.iter_mut() {
                    *v += amount;
                }
            }
            // ... other operations
        }
        Ok(())
    }
}
```

### Checkpoint Strategy

| Trigger | Description | Trade-off |
|---------|-------------|-----------|
| Delta count | Checkpoint every N deltas | Space vs. composition time |
| Time interval | Checkpoint every T seconds | Predictable latency |
| Composition cost | When compose > threshold | Adaptive optimization |
| Explicit request | On compact() or flush() | Manual control |

Default policy:
- Checkpoint at 100 deltas OR
- Checkpoint at 60 seconds OR
- When composition would exceed 1ms

---

## Consequences

### Benefits

1. **Network Efficiency**: 10-100x bandwidth reduction for sparse updates
2. **Temporal Queries**: Full history access, rollback, and audit
3. **Concurrent Updates**: CRDT semantics enable parallel writers
4. **Write Amplification**: Reduced through delta batching
5. **Index Stability**: Lazy repair reduces reorganization

### Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Composition overhead | Medium | Medium | Aggressive checkpointing, caching |
| Delta chain unbounded growth | Medium | High | Compaction policies |
| Conflict resolution correctness | Low | High | Formal CRDT verification |
| Index quality degradation | Medium | Medium | Quality bounds, forced repair |

### Performance Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| Delta application | < 50us | Must be faster than full write |
| Composition (100 deltas) | < 1ms | Acceptable read overhead |
| Checkpoint creation | < 10ms | Background operation |
| Network reduction (sparse) | > 10x | For <10% dimension changes |

---

## Implementation Phases

### Phase 1: Core Delta Infrastructure
- Delta types and storage
- Basic composition
- Simple checkpointing

### Phase 2: Propagation and Conflict Resolution
- Reactive push system
- CRDT implementation
- Causal ordering

### Phase 3: Index Integration
- Lazy HNSW repair
- Quality monitoring
- Incremental updates

### Phase 4: Optimization
- Advanced encoding
- Compression tiers
- Adaptive policies

---

## References

1. Shapiro, M., et al. "Conflict-free Replicated Data Types." SSS 2011.
2. Kleppmann, M. "Designing Data-Intensive Applications." O'Reilly, 2017.
3. ADR-001: Ruvector Core Architecture
4. ADR-CE-002: Incremental Coherence Computation

---

## Related Decisions

- **ADR-DB-002**: Delta Encoding Format
- **ADR-DB-003**: Delta Propagation Protocol
- **ADR-DB-004**: Delta Conflict Resolution
- **ADR-DB-005**: Delta Index Updates
- **ADR-DB-006**: Delta Compression Strategy
- **ADR-DB-007**: Delta Temporal Windows
- **ADR-DB-008**: Delta WASM Integration
- **ADR-DB-009**: Delta Observability
- **ADR-DB-010**: Delta Security Model
