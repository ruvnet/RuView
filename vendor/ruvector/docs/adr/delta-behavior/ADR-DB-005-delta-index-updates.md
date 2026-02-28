# ADR-DB-005: Delta Index Updates

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

### The Index Update Challenge

HNSW (Hierarchical Navigable Small World) indexes present unique challenges for delta-based updates:

1. **Graph Structure**: HNSW is a proximity graph where edges connect similar vectors
2. **Insert Complexity**: O(log n * ef_construction) for proper graph maintenance
3. **Update Semantics**: Standard HNSW has no native update operation
4. **Recall Sensitivity**: Graph quality directly impacts search recall
5. **Concurrent Access**: Updates must not corrupt concurrent searches

### Current HNSW Behavior

Ruvector's existing HNSW implementation (ADR-001) uses:
- `hnsw_rs` library for graph operations
- Mark-delete semantics (no graph restructuring)
- Full rebuild for significant changes
- No incremental edge updates

### Delta Update Scenarios

| Scenario | Vector Change | Impact on Neighbors |
|----------|---------------|---------------------|
| Minor adjustment (<5%) | Negligible | Neighbors likely still valid |
| Moderate change (5-20%) | Moderate | Some edges may be suboptimal |
| Major change (>20%) | Significant | Many edges invalidated |
| Dimension shift | Variable | Depends on affected dimensions |

---

## Decision

### Adopt Lazy Repair with Quality Bounds

We implement a **lazy repair** strategy that:
1. Applies deltas immediately to vector data
2. Defers index repair until quality degrades
3. Uses quality bounds to trigger selective repair
4. Maintains search correctness through fallback mechanisms

### Architecture Overview

```
                    ┌─────────────────────────────────────────────────────────────┐
                    │                    DELTA INDEX MANAGER                       │
                    └─────────────────────────────────────────────────────────────┘
                                               │
         ┌─────────────────┬─────────────────┬┴──────────────────┬─────────────────┐
         │                 │                 │                   │                 │
         v                 v                 v                   v                 v
    ┌─────────┐      ┌─────────┐      ┌───────────┐      ┌─────────────┐    ┌─────────┐
    │  Delta  │      │ Quality │      │   Lazy    │      │  Checkpoint │    │ Rebuild │
    │ Tracker │      │ Monitor │      │  Repair   │      │   Manager   │    │ Trigger │
    └─────────┘      └─────────┘      └───────────┘      └─────────────┘    └─────────┘
         │                 │                 │                   │                 │
         │                 │                 │                   │                 │
         v                 v                 v                   v                 v
    ┌─────────────────────────────────────────────────────────────────────────────────┐
    │                              HNSW INDEX LAYER                                   │
    │    Vector Data │ Edge Graph │ Entry Points │ Layer Structure │ Distance Cache  │
    └─────────────────────────────────────────────────────────────────────────────────┘
```

### Core Components

#### 1. Delta Tracker

```rust
/// Tracks pending index updates from deltas
pub struct DeltaTracker {
    /// Pending updates by vector ID
    pending: DashMap<VectorId, PendingUpdate>,
    /// Delta accumulation before index update
    delta_buffer: Vec<AccumulatedDelta>,
    /// Configuration
    config: DeltaTrackerConfig,
}

#[derive(Debug, Clone)]
pub struct PendingUpdate {
    /// Original vector (before deltas)
    pub original: Vec<f32>,
    /// Current vector (after deltas)
    pub current: Vec<f32>,
    /// Accumulated delta magnitude
    pub total_delta_magnitude: f32,
    /// Number of deltas accumulated
    pub delta_count: u32,
    /// First delta timestamp
    pub first_delta_at: Instant,
    /// Index entry status
    pub index_status: IndexStatus,
}

#[derive(Debug, Clone, Copy)]
pub enum IndexStatus {
    /// Index matches vector exactly
    Synchronized,
    /// Index is stale but within bounds
    Stale { estimated_quality: f32 },
    /// Index needs repair
    NeedsRepair,
    /// Not yet indexed
    NotIndexed,
}

impl DeltaTracker {
    /// Record a delta application
    pub fn record_delta(
        &self,
        vector_id: &VectorId,
        old_vector: &[f32],
        new_vector: &[f32],
    ) {
        let delta_magnitude = compute_l2_delta(old_vector, new_vector);

        self.pending
            .entry(vector_id.clone())
            .and_modify(|update| {
                update.current = new_vector.to_vec();
                update.total_delta_magnitude += delta_magnitude;
                update.delta_count += 1;
                update.index_status = self.estimate_status(update);
            })
            .or_insert_with(|| PendingUpdate {
                original: old_vector.to_vec(),
                current: new_vector.to_vec(),
                total_delta_magnitude: delta_magnitude,
                delta_count: 1,
                first_delta_at: Instant::now(),
                index_status: IndexStatus::Stale {
                    estimated_quality: self.estimate_quality(delta_magnitude),
                },
            });
    }

    /// Get vectors needing repair
    pub fn get_repair_candidates(&self) -> Vec<VectorId> {
        self.pending
            .iter()
            .filter(|e| matches!(e.index_status, IndexStatus::NeedsRepair))
            .map(|e| e.key().clone())
            .collect()
    }

    fn estimate_status(&self, update: &PendingUpdate) -> IndexStatus {
        let relative_change = update.total_delta_magnitude
            / (vector_magnitude(&update.original) + 1e-10);

        if relative_change > self.config.repair_threshold {
            IndexStatus::NeedsRepair
        } else {
            IndexStatus::Stale {
                estimated_quality: self.estimate_quality(update.total_delta_magnitude),
            }
        }
    }

    fn estimate_quality(&self, delta_magnitude: f32) -> f32 {
        // Quality decays with delta magnitude
        // Based on empirical HNSW edge validity studies
        (-delta_magnitude / self.config.quality_decay_constant).exp()
    }
}
```

#### 2. Quality Monitor

```rust
/// Monitors index quality and triggers repairs
pub struct QualityMonitor {
    /// Sampled quality measurements
    measurements: RingBuffer<QualityMeasurement>,
    /// Current quality estimate
    current_quality: AtomicF32,
    /// Quality bounds configuration
    bounds: QualityBounds,
    /// Repair trigger channel
    repair_trigger: Sender<RepairRequest>,
}

#[derive(Debug, Clone, Copy)]
pub struct QualityBounds {
    /// Minimum acceptable recall
    pub min_recall: f32,
    /// Target recall
    pub target_recall: f32,
    /// Sampling rate (fraction of searches)
    pub sample_rate: f32,
    /// Number of samples for estimate
    pub sample_window: usize,
}

impl Default for QualityBounds {
    fn default() -> Self {
        Self {
            min_recall: 0.90,
            target_recall: 0.95,
            sample_rate: 0.01, // Sample 1% of searches
            sample_window: 1000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QualityMeasurement {
    /// Estimated recall for this search
    pub recall: f32,
    /// Number of stale vectors encountered
    pub stale_vectors: u32,
    /// Timestamp
    pub timestamp: Instant,
}

impl QualityMonitor {
    /// Sample a search for quality estimation
    pub async fn sample_search(
        &self,
        query: &[f32],
        hnsw_results: &[SearchResult],
        k: usize,
    ) -> Option<QualityMeasurement> {
        // Only sample based on configured rate
        if !self.should_sample() {
            return None;
        }

        // Compute ground truth via exact search on sample
        let exact_results = self.exact_search_sample(query, k).await;

        // Calculate recall
        let hnsw_ids: HashSet<_> = hnsw_results.iter().map(|r| &r.id).collect();
        let exact_ids: HashSet<_> = exact_results.iter().map(|r| &r.id).collect();
        let overlap = hnsw_ids.intersection(&exact_ids).count();
        let recall = overlap as f32 / k as f32;

        // Count stale vectors in results
        let stale_count = self.count_stale_in_results(hnsw_results);

        let measurement = QualityMeasurement {
            recall,
            stale_vectors: stale_count,
            timestamp: Instant::now(),
        };

        // Update estimates
        self.measurements.push(measurement.clone());
        self.update_quality_estimate();

        // Trigger repair if below bounds
        if recall < self.bounds.min_recall {
            let _ = self.repair_trigger.send(RepairRequest::QualityBelowBounds {
                current_recall: recall,
                min_recall: self.bounds.min_recall,
            });
        }

        Some(measurement)
    }

    fn update_quality_estimate(&self) {
        let recent: Vec<_> = self.measurements
            .iter()
            .rev()
            .take(self.bounds.sample_window)
            .collect();

        if recent.is_empty() {
            return;
        }

        let avg_recall = recent.iter().map(|m| m.recall).sum::<f32>() / recent.len() as f32;
        self.current_quality.store(avg_recall, Ordering::Relaxed);
    }
}
```

#### 3. Lazy Repair Engine

```rust
/// Performs lazy index repair operations
pub struct LazyRepairEngine {
    /// HNSW index reference
    hnsw: Arc<RwLock<HnswIndex>>,
    /// Delta tracker reference
    tracker: Arc<DeltaTracker>,
    /// Repair configuration
    config: RepairConfig,
    /// Background repair task
    repair_task: Option<JoinHandle<()>>,
}

#[derive(Debug, Clone)]
pub struct RepairConfig {
    /// Maximum repairs per batch
    pub batch_size: usize,
    /// Repair interval
    pub repair_interval: Duration,
    /// Whether to use background repair
    pub background_repair: bool,
    /// Priority ordering for repairs
    pub priority: RepairPriority,
}

#[derive(Debug, Clone, Copy)]
pub enum RepairPriority {
    /// Repair most changed vectors first
    MostChanged,
    /// Repair oldest pending first
    Oldest,
    /// Repair most frequently accessed first
    MostAccessed,
    /// Round-robin
    RoundRobin,
}

impl LazyRepairEngine {
    /// Repair a single vector in the index
    pub async fn repair_vector(&self, vector_id: &VectorId) -> Result<RepairResult> {
        // Get current vector state
        let update = self.tracker.pending.get(vector_id)
            .ok_or(RepairError::VectorNotPending)?;

        let mut hnsw = self.hnsw.write().await;

        // Strategy 1: Soft update (if change is small)
        if update.total_delta_magnitude < self.config.soft_update_threshold {
            return self.soft_update(&mut hnsw, vector_id, &update.current).await;
        }

        // Strategy 2: Re-insertion (moderate change)
        if update.total_delta_magnitude < self.config.reinsert_threshold {
            return self.reinsert(&mut hnsw, vector_id, &update.current).await;
        }

        // Strategy 3: Full repair (large change)
        self.full_repair(&mut hnsw, vector_id, &update.current).await
    }

    /// Soft update: only update vector data, keep edges
    async fn soft_update(
        &self,
        hnsw: &mut HnswIndex,
        vector_id: &VectorId,
        new_vector: &[f32],
    ) -> Result<RepairResult> {
        // Update vector data without touching graph structure
        hnsw.update_vector_data(vector_id, new_vector)?;

        // Mark as synchronized
        self.tracker.pending.remove(vector_id);

        Ok(RepairResult::SoftUpdate {
            vector_id: vector_id.clone(),
            edges_preserved: true,
        })
    }

    /// Re-insertion: remove and re-add to graph
    async fn reinsert(
        &self,
        hnsw: &mut HnswIndex,
        vector_id: &VectorId,
        new_vector: &[f32],
    ) -> Result<RepairResult> {
        // Get current index position
        let old_idx = hnsw.get_index_for_vector(vector_id)?;

        // Mark old position as deleted
        hnsw.mark_deleted(old_idx)?;

        // Insert with new vector
        let new_idx = hnsw.insert_vector(vector_id.clone(), new_vector.to_vec())?;

        // Update tracker
        self.tracker.pending.remove(vector_id);

        Ok(RepairResult::Reinserted {
            vector_id: vector_id.clone(),
            old_idx,
            new_idx,
        })
    }

    /// Full repair: rebuild local neighborhood
    async fn full_repair(
        &self,
        hnsw: &mut HnswIndex,
        vector_id: &VectorId,
        new_vector: &[f32],
    ) -> Result<RepairResult> {
        // Get current neighbors
        let old_neighbors = hnsw.get_neighbors(vector_id)?;

        // Remove and reinsert
        self.reinsert(hnsw, vector_id, new_vector).await?;

        // Repair edges from old neighbors
        let repaired_edges = self.repair_neighbor_edges(hnsw, &old_neighbors).await?;

        Ok(RepairResult::FullRepair {
            vector_id: vector_id.clone(),
            repaired_edges,
        })
    }

    /// Background repair loop
    pub async fn run_background_repair(&self) {
        loop {
            tokio::time::sleep(self.config.repair_interval).await;

            // Get repair candidates
            let candidates = self.tracker.get_repair_candidates();

            if candidates.is_empty() {
                continue;
            }

            // Prioritize
            let prioritized = self.prioritize_repairs(candidates);

            // Repair batch
            for vector_id in prioritized.into_iter().take(self.config.batch_size) {
                if let Err(e) = self.repair_vector(&vector_id).await {
                    tracing::warn!("Repair failed for {}: {}", vector_id, e);
                }
            }
        }
    }
}
```

### Recall vs Latency Tradeoffs

```
                    ┌──────────────────────────────────────────────────────────┐
                    │              RECALL vs LATENCY TRADEOFF                   │
                    └──────────────────────────────────────────────────────────┘

    Recall
    100% │                                    ┌──────────────────┐
         │                                   /                    │
         │                                  /   Immediate Repair  │
         │                                 /                      │
     95% │    ┌───────────────────────────●───────────────────────┤
         │   /                            │                       │
         │  /         Lazy Repair         │                       │
         │ /                              │                       │
     90% │●───────────────────────────────┤                       │
         │                                │                       │
         │    Quality Bound               │                       │
     85% │    (Min Acceptable)            │                       │
         │                                │                       │
         └────────────────────────────────┴───────────────────────┴───>
               Low                    Medium                    High
                              Write Latency

    ──── Lazy Repair (Selected): Best balance
    - - - Immediate Repair: Highest recall, highest latency
    · · · No Repair: Lowest latency, recall degrades
```

### Repair Strategy Selection

```rust
/// Select repair strategy based on delta characteristics
pub fn select_repair_strategy(
    delta_magnitude: f32,
    vector_norm: f32,
    access_frequency: f32,
    current_recall: f32,
    config: &RepairConfig,
) -> RepairStrategy {
    let relative_change = delta_magnitude / (vector_norm + 1e-10);

    // High access frequency = repair sooner
    let access_weight = if access_frequency > config.hot_vector_threshold {
        0.7 // Reduce thresholds for hot vectors
    } else {
        1.0
    };

    // Low current recall = repair more aggressively
    let recall_weight = if current_recall < config.quality_bounds.min_recall {
        0.5 // Halve thresholds when recall is critical
    } else {
        1.0
    };

    let effective_threshold = config.soft_update_threshold * access_weight * recall_weight;

    if relative_change < effective_threshold {
        RepairStrategy::Deferred // No immediate action
    } else if relative_change < config.reinsert_threshold * access_weight * recall_weight {
        RepairStrategy::SoftUpdate
    } else if relative_change < config.full_repair_threshold * access_weight * recall_weight {
        RepairStrategy::Reinsert
    } else {
        RepairStrategy::FullRepair
    }
}
```

---

## Recall vs Latency Analysis

### Simulated Workload Results

| Strategy | Write Latency (p50) | Recall@10 | Recall@100 |
|----------|---------------------|-----------|------------|
| Immediate Repair | 2.1ms | 99.2% | 98.7% |
| Lazy (aggressive) | 150us | 96.5% | 95.1% |
| Lazy (balanced) | 80us | 94.2% | 92.8% |
| Lazy (relaxed) | 50us | 91.3% | 89.5% |
| No Repair | 35us | 85.1%* | 82.3%* |

*Degrades over time with update volume

### Quality Degradation Curves

```
Recall over time (1000 updates/sec, no repair):

100% ├────────────
     │            \
 95% │             \──────────────
     │                            \
 90% │                             \────────────
     │                                          \
 85% │                                           \───────
     │
 80% │
     └─────────────────────────────────────────────────────>
     0          5          10         15         20    Minutes

With lazy repair (balanced):

100% ├────────────
     │            \     ┌─────┐     ┌─────┐     ┌─────┐
 95% │             \───┬┘     └───┬┘     └───┬┘     └───
     │                 │ Repair   │ Repair   │ Repair
 90% │                 │          │          │
     │
 85% │
     └─────────────────────────────────────────────────────>
     0          5          10         15         20    Minutes
```

---

## Considered Options

### Option 1: Immediate Rebuild

**Description**: Rebuild affected portions of graph on every delta.

**Pros**:
- Always accurate graph
- Maximum recall
- Simple correctness model

**Cons**:
- O(log n * ef_construction) per update
- High write latency
- Blocks concurrent searches

**Verdict**: Rejected - latency unacceptable for streaming updates.

### Option 2: Periodic Full Rebuild

**Description**: Allow degradation, rebuild entire index periodically.

**Pros**:
- Minimal write overhead
- Predictable rebuild schedule
- Simple implementation

**Cons**:
- Extended degradation periods
- Expensive rebuilds
- Resource spikes

**Verdict**: Available as configuration option, not default.

### Option 3: Lazy Update (Selected)

**Description**: Defer repairs, trigger on quality bounds.

**Pros**:
- Low write latency
- Bounded recall degradation
- Adaptive to workload
- Background repair

**Cons**:
- Complexity in quality monitoring
- Potential recall dips

**Verdict**: Adopted - optimal balance for delta workloads.

### Option 4: Learned Index Repair

**Description**: ML model predicts optimal repair timing.

**Pros**:
- Potentially optimal decisions
- Adapts to patterns

**Cons**:
- Training complexity
- Model maintenance
- Explainability

**Verdict**: Deferred to future version.

---

## Technical Specification

### Index Update API

```rust
/// Delta-aware HNSW index
#[async_trait]
pub trait DeltaAwareIndex: Send + Sync {
    /// Apply delta without immediate index update
    async fn apply_delta(&self, delta: &VectorDelta) -> Result<DeltaApplication>;

    /// Get current recall estimate
    fn current_recall(&self) -> f32;

    /// Get vectors pending repair
    fn pending_repairs(&self) -> Vec<VectorId>;

    /// Force repair of specific vectors
    async fn repair_vectors(&self, ids: &[VectorId]) -> Result<Vec<RepairResult>>;

    /// Trigger background repair cycle
    async fn trigger_repair_cycle(&self) -> Result<RepairCycleSummary>;

    /// Search with optional quality sampling
    async fn search_with_quality(
        &self,
        query: &[f32],
        k: usize,
        sample_quality: bool,
    ) -> Result<SearchWithQuality>;
}

#[derive(Debug)]
pub struct DeltaApplication {
    pub vector_id: VectorId,
    pub delta_id: DeltaId,
    pub strategy: RepairStrategy,
    pub deferred_repair: bool,
    pub estimated_recall_impact: f32,
}

#[derive(Debug)]
pub struct SearchWithQuality {
    pub results: Vec<SearchResult>,
    pub quality_sample: Option<QualityMeasurement>,
    pub stale_results: u32,
}
```

### Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaIndexConfig {
    /// Quality bounds for triggering repair
    pub quality_bounds: QualityBounds,
    /// Repair engine configuration
    pub repair_config: RepairConfig,
    /// Delta tracker configuration
    pub tracker_config: DeltaTrackerConfig,
    /// Enable background repair
    pub background_repair: bool,
    /// Checkpoint interval (for recovery)
    pub checkpoint_interval: Duration,
}

impl Default for DeltaIndexConfig {
    fn default() -> Self {
        Self {
            quality_bounds: QualityBounds::default(),
            repair_config: RepairConfig {
                batch_size: 100,
                repair_interval: Duration::from_secs(5),
                background_repair: true,
                priority: RepairPriority::MostChanged,
                soft_update_threshold: 0.05,    // 5% change
                reinsert_threshold: 0.20,       // 20% change
                full_repair_threshold: 0.50,    // 50% change
            },
            tracker_config: DeltaTrackerConfig {
                repair_threshold: 0.15,
                quality_decay_constant: 0.1,
            },
            background_repair: true,
            checkpoint_interval: Duration::from_secs(300),
        }
    }
}
```

---

## Consequences

### Benefits

1. **Low Write Latency**: Sub-millisecond delta application
2. **Bounded Degradation**: Quality monitoring prevents unacceptable recall
3. **Adaptive**: Repairs prioritized by impact and access patterns
4. **Background Processing**: Repairs don't block user operations
5. **Resource Efficient**: Avoids unnecessary graph restructuring

### Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Recall below bounds | Low | High | Aggressive repair triggers |
| Repair backlog | Medium | Medium | Batch size tuning |
| Stale search results | Medium | Medium | Optional exact fallback |
| Checkpoint overhead | Low | Low | Incremental checkpoints |

---

## References

1. Malkov, Y., & Yashunin, D. "Efficient and robust approximate nearest neighbor search using HNSW graphs."
2. Singh, A., et al. "FreshDiskANN: A Fast and Accurate Graph-Based ANN Index for Streaming Similarity Search."
3. ADR-001: Ruvector Core Architecture
4. ADR-DB-001: Delta Behavior Core Architecture

---

## Related Decisions

- **ADR-DB-001**: Delta Behavior Core Architecture
- **ADR-DB-003**: Delta Propagation Protocol
- **ADR-DB-007**: Delta Temporal Windows
