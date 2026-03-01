# ADR-DB-004: Delta Conflict Resolution

**Status**: Proposed
**Date**: 2026-01-28
**Authors**: RuVector Architecture Team
**Deciders**: Architecture Review Board
**Parent**: ADR-DB-001 Delta Behavior Core Architecture

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-01-28 | Architecture Team | Initial proposal |

---

## Context and Problem Statement

### The Conflict Challenge

In distributed delta-first systems, concurrent updates to the same vector can create conflicts:

```
    Time ─────────────────────────────────────────>

    Replica A:  v0 ──[Δa: dim[5]=0.8]──> v1a
                  \
                   \
    Replica B:      ──[Δb: dim[5]=0.3]──> v1b

    Conflict: Both replicas modified dim[5] concurrently
```

### Conflict Scenarios

| Scenario | Frequency | Complexity |
|----------|-----------|------------|
| Same dimension, different values | High | Simple |
| Overlapping sparse updates | Medium | Moderate |
| Scale vs. sparse conflict | Low | Complex |
| Delete vs. update race | Low | Critical |

### Requirements

1. **Deterministic**: Same conflicts resolve identically on all replicas
2. **Commutative**: Order of conflict discovery doesn't affect outcome
3. **Low Latency**: Resolution shouldn't block writes
4. **Meaningful**: Results should be mathematically sensible for vectors

---

## Decision

### Adopt CRDT-Based Resolution with Causal Ordering

We implement conflict resolution using Conflict-free Replicated Data Types (CRDTs) with vector-specific merge semantics.

### CRDT Design for Vectors

#### Vector as a CRDT

```rust
/// CRDT-enabled vector with per-dimension version tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrdtVector {
    /// Vector ID
    pub id: VectorId,
    /// Dimensions with per-dimension causality
    pub dimensions: Vec<CrdtDimension>,
    /// Overall vector clock
    pub clock: VectorClock,
    /// Deletion marker
    pub tombstone: Option<Tombstone>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrdtDimension {
    /// Current value
    pub value: f32,
    /// Last update clock
    pub clock: VectorClock,
    /// Originating replica
    pub origin: ReplicaId,
    /// Timestamp of update
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tombstone {
    pub deleted_at: DateTime<Utc>,
    pub deleted_by: ReplicaId,
    pub clock: VectorClock,
}
```

#### Merge Operation

```rust
impl CrdtVector {
    /// Merge another CRDT vector into this one
    pub fn merge(&mut self, other: &CrdtVector) -> MergeResult {
        assert_eq!(self.id, other.id);
        let mut conflicts = Vec::new();

        // Handle tombstone
        self.tombstone = match (&self.tombstone, &other.tombstone) {
            (None, None) => None,
            (Some(t), None) | (None, Some(t)) => Some(t.clone()),
            (Some(t1), Some(t2)) => {
                // Latest tombstone wins
                Some(if t1.timestamp > t2.timestamp { t1.clone() } else { t2.clone() })
            }
        };

        // If deleted, no need to merge dimensions
        if self.tombstone.is_some() {
            return MergeResult { conflicts, tombstoned: true };
        }

        // Merge each dimension
        for (i, (self_dim, other_dim)) in
            self.dimensions.iter_mut().zip(other.dimensions.iter()).enumerate()
        {
            let ordering = self_dim.clock.compare(&other_dim.clock);

            match ordering {
                ClockOrdering::Before => {
                    // Other is newer, take it
                    *self_dim = other_dim.clone();
                }
                ClockOrdering::After | ClockOrdering::Equal => {
                    // Self is newer or equal, keep it
                }
                ClockOrdering::Concurrent => {
                    // Conflict! Apply resolution strategy
                    let resolved = self.resolve_dimension_conflict(i, self_dim, other_dim);
                    conflicts.push(DimensionConflict {
                        dimension: i,
                        local_value: self_dim.value,
                        remote_value: other_dim.value,
                        resolved_value: resolved.value,
                        strategy: resolved.strategy,
                    });
                    *self_dim = resolved.dimension;
                }
            }
        }

        // Update overall clock
        self.clock.merge(&other.clock);

        MergeResult { conflicts, tombstoned: false }
    }

    fn resolve_dimension_conflict(
        &self,
        dim_idx: usize,
        local: &CrdtDimension,
        remote: &CrdtDimension,
    ) -> ResolvedDimension {
        // Strategy selection based on configured policy
        match self.conflict_strategy(dim_idx) {
            ConflictStrategy::LastWriteWins => {
                // Latest timestamp wins
                let winner = if local.timestamp > remote.timestamp { local } else { remote };
                ResolvedDimension {
                    dimension: winner.clone(),
                    strategy: ConflictStrategy::LastWriteWins,
                }
            }
            ConflictStrategy::MaxValue => {
                // Take maximum value
                let max_val = local.value.max(remote.value);
                let winner = if local.value >= remote.value { local } else { remote };
                ResolvedDimension {
                    dimension: CrdtDimension {
                        value: max_val,
                        clock: merge_clocks(&local.clock, &remote.clock),
                        origin: winner.origin.clone(),
                        timestamp: winner.timestamp.max(remote.timestamp),
                    },
                    strategy: ConflictStrategy::MaxValue,
                }
            }
            ConflictStrategy::Average => {
                // Average the values
                let avg = (local.value + remote.value) / 2.0;
                ResolvedDimension {
                    dimension: CrdtDimension {
                        value: avg,
                        clock: merge_clocks(&local.clock, &remote.clock),
                        origin: "merged".into(),
                        timestamp: local.timestamp.max(remote.timestamp),
                    },
                    strategy: ConflictStrategy::Average,
                }
            }
            ConflictStrategy::ReplicaPriority(priorities) => {
                // Higher priority replica wins
                let local_priority = priorities.get(&local.origin).copied().unwrap_or(0);
                let remote_priority = priorities.get(&remote.origin).copied().unwrap_or(0);
                let winner = if local_priority >= remote_priority { local } else { remote };
                ResolvedDimension {
                    dimension: winner.clone(),
                    strategy: ConflictStrategy::ReplicaPriority(priorities),
                }
            }
        }
    }
}
```

### Conflict Resolution Strategies

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictStrategy {
    /// Last write wins based on timestamp
    LastWriteWins,
    /// Take maximum value (for monotonic dimensions)
    MaxValue,
    /// Take minimum value
    MinValue,
    /// Average conflicting values
    Average,
    /// Weighted average based on replica trust
    WeightedAverage(HashMap<ReplicaId, f32>),
    /// Replica priority ordering
    ReplicaPriority(HashMap<ReplicaId, u32>),
    /// Custom merge function
    Custom(CustomMergeFn),
}

pub type CustomMergeFn = Arc<dyn Fn(f32, f32, &ConflictContext) -> f32 + Send + Sync>;
```

### Vector Clock Implementation

```rust
/// Extended vector clock for delta tracking
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorClock {
    /// Replica -> logical timestamp mapping
    clock: HashMap<ReplicaId, u64>,
}

impl VectorClock {
    pub fn new() -> Self {
        Self { clock: HashMap::new() }
    }

    /// Increment for local replica
    pub fn increment(&mut self, replica: &ReplicaId) {
        let counter = self.clock.entry(replica.clone()).or_insert(0);
        *counter += 1;
    }

    /// Get timestamp for replica
    pub fn get(&self, replica: &ReplicaId) -> u64 {
        self.clock.get(replica).copied().unwrap_or(0)
    }

    /// Merge with another clock (take max)
    pub fn merge(&mut self, other: &VectorClock) {
        for (replica, &ts) in &other.clock {
            let current = self.clock.entry(replica.clone()).or_insert(0);
            *current = (*current).max(ts);
        }
    }

    /// Compare two clocks for causality
    pub fn compare(&self, other: &VectorClock) -> ClockOrdering {
        let mut less_than = false;
        let mut greater_than = false;

        // Check all replicas in self
        for (replica, &self_ts) in &self.clock {
            let other_ts = other.get(replica);
            if self_ts < other_ts {
                less_than = true;
            } else if self_ts > other_ts {
                greater_than = true;
            }
        }

        // Check replicas only in other
        for (replica, &other_ts) in &other.clock {
            if !self.clock.contains_key(replica) && other_ts > 0 {
                less_than = true;
            }
        }

        match (less_than, greater_than) {
            (false, false) => ClockOrdering::Equal,
            (true, false) => ClockOrdering::Before,
            (false, true) => ClockOrdering::After,
            (true, true) => ClockOrdering::Concurrent,
        }
    }

    /// Check if concurrent (conflicting)
    pub fn is_concurrent(&self, other: &VectorClock) -> bool {
        matches!(self.compare(other), ClockOrdering::Concurrent)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockOrdering {
    Equal,
    Before,
    After,
    Concurrent,
}
```

### Operation-Based Delta Merging

```rust
/// Merge concurrent delta operations
pub fn merge_delta_operations(
    local: &DeltaOperation,
    remote: &DeltaOperation,
    strategy: &ConflictStrategy,
) -> DeltaOperation {
    match (local, remote) {
        // Both sparse: merge index sets
        (
            DeltaOperation::Sparse { indices: li, values: lv },
            DeltaOperation::Sparse { indices: ri, values: rv },
        ) => {
            let mut merged_indices = Vec::new();
            let mut merged_values = Vec::new();

            let local_map: HashMap<_, _> = li.iter().zip(lv.iter()).collect();
            let remote_map: HashMap<_, _> = ri.iter().zip(rv.iter()).collect();

            let all_indices: HashSet<_> = li.iter().chain(ri.iter()).collect();

            for &idx in all_indices {
                let local_val = local_map.get(&idx).copied();
                let remote_val = remote_map.get(&idx).copied();

                let value = match (local_val, remote_val) {
                    (Some(&l), None) => l,
                    (None, Some(&r)) => r,
                    (Some(&l), Some(&r)) => resolve_value_conflict(l, r, strategy),
                    (None, None) => unreachable!(),
                };

                merged_indices.push(*idx);
                merged_values.push(value);
            }

            DeltaOperation::Sparse {
                indices: merged_indices,
                values: merged_values,
            }
        }

        // Sparse vs Dense: apply sparse changes on top of dense
        (
            DeltaOperation::Sparse { indices, values },
            DeltaOperation::Dense { vector },
        )
        | (
            DeltaOperation::Dense { vector },
            DeltaOperation::Sparse { indices, values },
        ) => {
            let mut result = vector.clone();
            for (&idx, &val) in indices.iter().zip(values.iter()) {
                result[idx as usize] = val;
            }
            DeltaOperation::Dense { vector: result }
        }

        // Both dense: element-wise merge
        (
            DeltaOperation::Dense { vector: lv },
            DeltaOperation::Dense { vector: rv },
        ) => {
            let merged: Vec<f32> = lv.iter()
                .zip(rv.iter())
                .map(|(&l, &r)| resolve_value_conflict(l, r, strategy))
                .collect();
            DeltaOperation::Dense { vector: merged }
        }

        // Scale operations: compose
        (
            DeltaOperation::Scale { factor: f1 },
            DeltaOperation::Scale { factor: f2 },
        ) => {
            DeltaOperation::Scale { factor: f1 * f2 }
        }

        // Delete wins over updates (tombstone semantics)
        (DeltaOperation::Delete, _) | (_, DeltaOperation::Delete) => {
            DeltaOperation::Delete
        }

        // Other combinations: convert to dense and merge
        _ => {
            // Fallback: materialize both and merge
            DeltaOperation::Dense {
                vector: vec![], // Would compute actual merge
            }
        }
    }
}

fn resolve_value_conflict(local: f32, remote: f32, strategy: &ConflictStrategy) -> f32 {
    match strategy {
        ConflictStrategy::LastWriteWins => remote, // Assume remote is "latest"
        ConflictStrategy::MaxValue => local.max(remote),
        ConflictStrategy::MinValue => local.min(remote),
        ConflictStrategy::Average => (local + remote) / 2.0,
        ConflictStrategy::WeightedAverage(weights) => {
            // Would need context for proper weighting
            (local + remote) / 2.0
        }
        _ => remote, // Default fallback
    }
}
```

---

## Consistency Guarantees

### Eventual Consistency

The CRDT approach guarantees **strong eventual consistency**:

1. **Eventual Delivery**: All deltas eventually reach all replicas
2. **Convergence**: Replicas with same deltas converge to same state
3. **Termination**: Merge operations always terminate

### Causal Consistency

Vector clocks ensure causal ordering:

```
Property: If Δa happens-before Δb, then on all replicas:
          Δa is applied before Δb

Proof: Vector clock comparison ensures causal dependencies
       are satisfied before applying deltas
```

### Conflict Freedom Theorem

```
For any two concurrent deltas Δa and Δb:
  merge(Δa, Δb) = merge(Δb, Δa)  [Commutativity]
  merge(Δa, merge(Δb, Δc)) = merge(merge(Δa, Δb), Δc)  [Associativity]
  merge(Δa, Δa) = Δa  [Idempotence]
```

These properties ensure:
- Order-independent convergence
- Safe retry/redelivery
- Partition tolerance

---

## Considered Options

### Option 1: Last-Write-Wins (LWW)

**Description**: Latest timestamp wins, simple conflict resolution.

**Pros**:
- Extremely simple
- Low overhead
- Deterministic

**Cons**:
- Clock skew sensitivity
- Loses concurrent updates
- No semantic awareness

**Verdict**: Available as strategy option, not default.

### Option 2: Pure Vector Clocks

**Description**: Track causality, reject concurrent writes.

**Pros**:
- Perfect causality tracking
- No data loss

**Cons**:
- Requires conflict handling at application level
- Concurrent writes fail

**Verdict**: Rejected - too restrictive for vector workloads.

### Option 3: Operational Transform (OT)

**Description**: Transform operations to maintain consistency.

**Pros**:
- Preserves all intentions
- Used successfully in collaborative editing

**Cons**:
- Complex transformation functions
- Hard to prove correctness
- Doesn't map well to vector semantics

**Verdict**: Rejected - CRDT semantics more natural for vectors.

### Option 4: CRDT with Causal Ordering (Selected)

**Description**: CRDT merge with per-dimension version tracking.

**Pros**:
- Automatic convergence
- Semantically meaningful merges
- Flexible strategies
- Proven correctness

**Cons**:
- Per-dimension overhead
- More complex than LWW

**Verdict**: Adopted - optimal balance of correctness and flexibility.

---

## Technical Specification

### Conflict Detection API

```rust
/// Detect conflicts between deltas
pub fn detect_conflicts(
    local_delta: &VectorDelta,
    remote_delta: &VectorDelta,
) -> ConflictReport {
    let mut conflicts = Vec::new();

    // Check if targeting same vector
    if local_delta.vector_id != remote_delta.vector_id {
        return ConflictReport::NoConflict;
    }

    // Check causality
    let ordering = local_delta.clock.compare(&remote_delta.clock);

    if ordering != ClockOrdering::Concurrent {
        return ConflictReport::Ordered { ordering };
    }

    // Analyze operation conflicts
    let op_conflicts = analyze_operation_conflicts(
        &local_delta.operation,
        &remote_delta.operation,
    );

    ConflictReport::Conflicts {
        vector_id: local_delta.vector_id.clone(),
        local_delta_id: local_delta.delta_id.clone(),
        remote_delta_id: remote_delta.delta_id.clone(),
        dimension_conflicts: op_conflicts,
    }
}
```

### Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictConfig {
    /// Default resolution strategy
    pub default_strategy: ConflictStrategy,
    /// Per-namespace strategies
    pub namespace_strategies: HashMap<String, ConflictStrategy>,
    /// Per-dimension strategies (dimension index -> strategy)
    pub dimension_strategies: HashMap<usize, ConflictStrategy>,
    /// Whether to log conflicts
    pub log_conflicts: bool,
    /// Conflict callback for custom handling
    #[serde(skip)]
    pub conflict_callback: Option<ConflictCallback>,
    /// Tombstone retention duration
    pub tombstone_retention: Duration,
}

impl Default for ConflictConfig {
    fn default() -> Self {
        Self {
            default_strategy: ConflictStrategy::LastWriteWins,
            namespace_strategies: HashMap::new(),
            dimension_strategies: HashMap::new(),
            log_conflicts: true,
            conflict_callback: None,
            tombstone_retention: Duration::from_secs(86400 * 7), // 7 days
        }
    }
}
```

---

## Consequences

### Benefits

1. **Automatic Convergence**: All replicas converge without coordination
2. **Partition Tolerance**: Works during network partitions
3. **Semantic Merging**: Vector-appropriate conflict resolution
4. **Flexibility**: Configurable per-dimension strategies
5. **Auditability**: All conflicts logged with resolution

### Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Memory overhead | Medium | Medium | Lazy per-dimension tracking |
| Merge complexity | Low | Medium | Thorough testing, formal verification |
| Strategy misconfiguration | Medium | High | Sensible defaults, validation |
| Tombstone accumulation | Medium | Medium | Garbage collection policies |

---

## References

1. Shapiro, M., et al. "Conflict-free Replicated Data Types." SSS 2011.
2. Kleppmann, M., & Almeida, P. S. "A Conflict-Free Replicated JSON Datatype." IEEE TPDS 2017.
3. Ruvector conflict.rs: Existing conflict resolution implementation
4. ADR-DB-001: Delta Behavior Core Architecture

---

## Related Decisions

- **ADR-DB-001**: Delta Behavior Core Architecture
- **ADR-DB-003**: Delta Propagation Protocol
- **ADR-DB-005**: Delta Index Updates
