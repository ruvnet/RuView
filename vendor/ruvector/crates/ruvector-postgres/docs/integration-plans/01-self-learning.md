# Self-Learning / ReasoningBank Integration Plan

## Overview

Integrate adaptive learning capabilities into ruvector-postgres, enabling the database to learn from query patterns, optimize search strategies, and improve recall/precision over time.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    PostgreSQL Extension                      │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  Trajectory │  │   Verdict   │  │  Memory Distillation│  │
│  │   Tracker   │  │   Judgment  │  │       Engine        │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
│         │                │                     │             │
│         └────────────────┼─────────────────────┘             │
│                          ▼                                   │
│              ┌───────────────────────┐                       │
│              │    ReasoningBank      │                       │
│              │   (Pattern Storage)   │                       │
│              └───────────────────────┘                       │
└─────────────────────────────────────────────────────────────┘
```

## Module Structure

```
src/
├── learning/
│   ├── mod.rs              # Module exports
│   ├── trajectory.rs       # Query trajectory tracking
│   ├── verdict.rs          # Success/failure judgment
│   ├── distillation.rs     # Pattern extraction
│   ├── reasoning_bank.rs   # Pattern storage & retrieval
│   └── optimizer.rs        # Search parameter optimization
```

## SQL Interface

### Configuration

```sql
-- Enable self-learning for a table
SELECT ruvector_enable_learning('embeddings',
    trajectory_window := 1000,
    learning_rate := 0.01,
    min_samples := 100
);

-- View learning statistics
SELECT * FROM ruvector_learning_stats('embeddings');

-- Export learned patterns
SELECT ruvector_export_patterns('embeddings') AS patterns_json;

-- Import patterns from another instance
SELECT ruvector_import_patterns('embeddings', patterns_json);
```

### Automatic Optimization

```sql
-- Auto-tune HNSW parameters based on query patterns
SELECT ruvector_auto_tune('embeddings_idx',
    optimize_for := 'recall',  -- or 'latency', 'balanced'
    sample_queries := 1000
);

-- Get recommended index parameters
SELECT * FROM ruvector_recommend_params('embeddings');
```

## Implementation Phases

### Phase 1: Trajectory Tracking (Week 1-2)

```rust
// src/learning/trajectory.rs

pub struct QueryTrajectory {
    pub query_id: Uuid,
    pub query_vector: Vec<f32>,
    pub timestamp: DateTime<Utc>,
    pub index_params: IndexParams,
    pub results: Vec<SearchResult>,
    pub latency_ms: f64,
    pub recall_estimate: Option<f32>,
}

pub struct TrajectoryTracker {
    buffer: RingBuffer<QueryTrajectory>,
    storage: TrajectoryStorage,
}

impl TrajectoryTracker {
    pub fn record(&mut self, trajectory: QueryTrajectory);
    pub fn get_recent(&self, n: usize) -> Vec<&QueryTrajectory>;
    pub fn analyze_patterns(&self) -> PatternAnalysis;
}
```

**SQL Functions:**
```sql
-- Record query feedback (user indicates relevance)
SELECT ruvector_record_feedback(
    query_id := 'abc123',
    relevant_ids := ARRAY[1, 5, 7],
    irrelevant_ids := ARRAY[2, 3]
);
```

### Phase 2: Verdict Judgment (Week 3-4)

```rust
// src/learning/verdict.rs

pub struct VerdictEngine {
    success_threshold: f32,
    metrics: VerdictMetrics,
}

impl VerdictEngine {
    /// Judge if a search was successful based on multiple signals
    pub fn judge(&self, trajectory: &QueryTrajectory) -> Verdict {
        let signals = vec![
            self.latency_score(trajectory),
            self.recall_score(trajectory),
            self.diversity_score(trajectory),
            self.user_feedback_score(trajectory),
        ];

        Verdict {
            success: signals.iter().sum::<f32>() / signals.len() as f32 > self.success_threshold,
            confidence: self.compute_confidence(&signals),
            recommendations: self.generate_recommendations(&signals),
        }
    }
}
```

### Phase 3: Memory Distillation (Week 5-6)

```rust
// src/learning/distillation.rs

pub struct DistillationEngine {
    pattern_extractor: PatternExtractor,
    compressor: PatternCompressor,
}

impl DistillationEngine {
    /// Extract reusable patterns from trajectories
    pub fn distill(&self, trajectories: &[QueryTrajectory]) -> Vec<LearnedPattern> {
        let raw_patterns = self.pattern_extractor.extract(trajectories);
        let compressed = self.compressor.compress(raw_patterns);
        compressed
    }
}

pub struct LearnedPattern {
    pub query_cluster_centroid: Vec<f32>,
    pub optimal_ef_search: u32,
    pub optimal_probes: u32,
    pub expected_recall: f32,
    pub confidence: f32,
}
```

### Phase 4: ReasoningBank Storage (Week 7-8)

```rust
// src/learning/reasoning_bank.rs

pub struct ReasoningBank {
    patterns: HnswIndex<LearnedPattern>,
    metadata: HashMap<PatternId, PatternMetadata>,
}

impl ReasoningBank {
    /// Find applicable patterns for a query
    pub fn lookup(&self, query: &[f32], k: usize) -> Vec<&LearnedPattern> {
        self.patterns.search(query, k)
    }

    /// Store a new pattern
    pub fn store(&mut self, pattern: LearnedPattern) -> PatternId;

    /// Merge similar patterns to prevent bloat
    pub fn consolidate(&mut self);

    /// Prune low-value patterns
    pub fn prune(&mut self, min_usage: u32, min_confidence: f32);
}
```

### Phase 5: Search Optimizer (Week 9-10)

```rust
// src/learning/optimizer.rs

pub struct SearchOptimizer {
    reasoning_bank: Arc<ReasoningBank>,
    default_params: SearchParams,
}

impl SearchOptimizer {
    /// Get optimized parameters for a query
    pub fn optimize(&self, query: &[f32]) -> SearchParams {
        match self.reasoning_bank.lookup(query, 3) {
            patterns if !patterns.is_empty() => {
                self.interpolate_params(query, patterns)
            }
            _ => self.default_params.clone()
        }
    }

    fn interpolate_params(&self, query: &[f32], patterns: &[&LearnedPattern]) -> SearchParams {
        // Weight patterns by similarity to query
        let weights: Vec<f32> = patterns.iter()
            .map(|p| cosine_similarity(query, &p.query_cluster_centroid))
            .collect();

        SearchParams {
            ef_search: weighted_average(
                patterns.iter().map(|p| p.optimal_ef_search as f32),
                &weights
            ) as u32,
            // ...
        }
    }
}
```

## PostgreSQL Integration

### Background Worker

```rust
// src/learning/bgworker.rs

#[pg_guard]
pub extern "C" fn learning_bgworker_main(_arg: pg_sys::Datum) {
    BackgroundWorker::attach_signal_handlers(SignalWakeFlags::SIGHUP | SignalWakeFlags::SIGTERM);

    loop {
        // Process trajectory buffer
        let trajectories = TRAJECTORY_BUFFER.drain();

        if trajectories.len() >= MIN_BATCH_SIZE {
            // Distill patterns
            let patterns = DISTILLATION_ENGINE.distill(&trajectories);

            // Store in reasoning bank
            for pattern in patterns {
                REASONING_BANK.store(pattern);
            }

            // Periodic consolidation
            if should_consolidate() {
                REASONING_BANK.consolidate();
            }
        }

        // Sleep until next batch
        BackgroundWorker::wait_latch(LEARNING_INTERVAL_MS);
    }
}
```

### GUC Configuration

```rust
static LEARNING_ENABLED: GucSetting<bool> = GucSetting::new(false);
static LEARNING_RATE: GucSetting<f64> = GucSetting::new(0.01);
static TRAJECTORY_BUFFER_SIZE: GucSetting<i32> = GucSetting::new(10000);
static PATTERN_CONSOLIDATION_INTERVAL: GucSetting<i32> = GucSetting::new(3600);
```

## Optimization Strategies

### 1. Adaptive ef_search

```sql
-- Before: Static ef_search
SET ruvector.ef_search = 40;
SELECT * FROM items ORDER BY embedding <-> query_vec LIMIT 10;

-- After: Adaptive ef_search based on learned patterns
SELECT * FROM items
ORDER BY embedding <-> query_vec
LIMIT 10
WITH (adaptive_search := true);
```

### 2. Query-Aware Probing

For IVFFlat, learn optimal probe counts per query cluster:

```rust
pub fn adaptive_probes(&self, query: &[f32]) -> u32 {
    let cluster_id = self.assign_cluster(query);
    self.learned_probes.get(&cluster_id).unwrap_or(&self.default_probes)
}
```

### 3. Index Selection

Learn when to use HNSW vs IVFFlat:

```rust
pub fn select_index(&self, query: &[f32], k: usize) -> IndexType {
    let features = QueryFeatures::extract(query, k);
    self.index_selector.predict(&features)
}
```

## Benchmarks

### Metrics to Track

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| Recall@10 | 0.95 | 0.98 | After 10K queries |
| p99 Latency | 5ms | 3ms | After learning |
| Memory Overhead | 0 | <100MB | Pattern storage |
| Learning Time | N/A | <1s/1K queries | Background processing |

### Benchmark Queries

```sql
-- Measure recall improvement
SELECT ruvector_benchmark_recall(
    table_name := 'embeddings',
    ground_truth_table := 'embeddings_ground_truth',
    num_queries := 1000,
    k := 10
);

-- Measure latency improvement
SELECT ruvector_benchmark_latency(
    table_name := 'embeddings',
    num_queries := 10000,
    k := 10,
    percentiles := ARRAY[50, 90, 99]
);
```

## Dependencies

```toml
[dependencies]
# Existing ruvector crates (optional integration)
# ruvector-core = { path = "../ruvector-core", optional = true }

# Pattern storage
dashmap = "6.0"
parking_lot = "0.12"

# Statistics
statrs = "0.16"

# Clustering for pattern extraction
linfa = "0.7"
linfa-clustering = "0.7"

# Serialization for pattern export/import
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## Feature Flags

```toml
[features]
learning = []
learning-advanced = ["learning", "linfa", "linfa-clustering"]
learning-distributed = ["learning", "ruvector-replication"]
```

## Migration Path

1. **v0.2.0**: Basic trajectory tracking, manual feedback
2. **v0.3.0**: Verdict judgment, automatic pattern extraction
3. **v0.4.0**: Full ReasoningBank, adaptive search
4. **v0.5.0**: Distributed learning across replicas

## Security Considerations

- Pattern data is stored locally, no external transmission
- Trajectory data can be anonymized (hash query vectors)
- Learning can be disabled per-table for sensitive data
- Export/import requires superuser privileges
