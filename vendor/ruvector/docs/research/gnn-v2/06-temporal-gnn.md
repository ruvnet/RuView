# Continuous-Time Dynamic Graph Neural Networks

## Overview

### Problem Statement

Traditional GNN embeddings are static snapshots that cannot capture temporal evolution of graphs. Real-world applications involve time-varying graphs where:

- **Node Embeddings Change**: User interests, document relevance, and product features evolve
- **Edge Dynamics**: Relationships form and dissolve (social connections, co-occurrence)
- **Temporal Patterns**: Seasonal trends, trending topics, time-sensitive queries
- **Staleness Issues**: Static embeddings become outdated, requiring full recomputation
- **Event Sequencing**: Order matters (buy → review vs. review → buy)

Current solutions either:
1. Retrain entire model periodically (expensive, disruptive)
2. Use discrete time snapshots (loses fine-grained dynamics)
3. Ignore temporal information (poor accuracy for time-sensitive tasks)

### Proposed Solution

Implement a **Continuous-Time Dynamic GNN (CTDGNN)** system with:

1. **Temporal Node Memory**: Exponentially decaying memory of past interactions
2. **Fourier Time Encoding**: Continuous time representation via sinusoidal functions
3. **Temporal Attention**: Attention weights modulated by time distance
4. **Incremental Updates**: Fast online updates without full retraining
5. **Time-Aware HNSW**: Index supports temporal queries ("similar to X at time T")

**Key Innovation**: Embeddings are functions of time `h_i(t)` rather than static vectors `h_i`.

### Expected Benefits

**Quantified Improvements**:
- **Accuracy**: 15-25% improvement on temporal prediction tasks
- **Freshness**: Real-time updates vs. hours/days for retraining
- **Update Speed**: 100-1000x faster than full retraining (microseconds vs. seconds)
- **Memory Efficiency**: 2-5x compression via temporal aggregation
- **Query Flexibility**: Support "what was similar to X yesterday" queries

**Use Cases**:
- Streaming recommendation (Netflix, Spotify)
- Financial fraud detection (transaction patterns)
- Social network analysis (trending topics)
- Document versioning (Wikipedia edits)
- Time-series forecasting

## Technical Design

### Architecture Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                   TemporalGNN<T>                                  │
├──────────────────────────────────────────────────────────────────┤
│  - base_embeddings: Vec<Vec<T>>           [Static component]      │
│  - temporal_memory: Vec<TemporalMemory>   [Dynamic component]     │
│  - time_encoder: FourierTimeEncoder                              │
│  - aggregator: TemporalAggregator                                │
│  - current_time: f64                      [Logical timestamp]     │
└──────────────────────────────────────────────────────────────────┘
                            ▲
                            │
        ┌───────────────────┴────────────────────┐
        │                                        │
┌───────▼────────────────┐           ┌──────────▼────────────┐
│ TemporalMemory         │           │ FourierTimeEncoder    │
├────────────────────────┤           ├───────────────────────┤
│ - events: RingBuffer   │           │ - frequencies: Vec<f64>│
│ - decay_rate: f32      │           │ - dimension: usize    │
│ - aggregated: Vec<T>   │           │                       │
│                        │           │ + encode(t) -> Vec<T> │
│ + update(event, t)     │           │ + decode(enc)-> f64   │
│ + get_at_time(t)       │           └───────────────────────┘
│ + decay()              │
└────────────────────────┘
        │
        ▼
┌────────────────────────┐           ┌───────────────────────┐
│ TemporalEvent          │           │ TemporalAttention     │
├────────────────────────┤           ├───────────────────────┤
│ - timestamp: f64       │           │ - time_window: f64    │
│ - value: Vec<T>        │           │ - decay_fn: DecayFn   │
│ - weight: f32          │           │                       │
│ - event_type: EventType│           │ + compute_weight()    │
└────────────────────────┘           └───────────────────────┘

┌──────────────────────────────────────────────────────────────────┐
│                   TemporalHNSW<T>                                 │
├──────────────────────────────────────────────────────────────────┤
│ - temporal_gnn: TemporalGNN<T>                                   │
│ - time_slices: BTreeMap<TimeRange, HNSWIndex>  [Indexed slices] │
│ - active_slice: HNSWIndex                      [Current time]    │
│                                                                  │
│ + search_at_time(query, t, k) -> Vec<Result>                    │
│ + search_time_range(query, t_start, t_end, k)                   │
│ + update_embedding(node_id, event, t)                           │
└──────────────────────────────────────────────────────────────────┘
```

### Core Data Structures

```rust
/// Temporal graph neural network with time-evolving embeddings
pub struct TemporalGNN<T: Float> {
    /// Static base embeddings (initial state)
    base_embeddings: Vec<Vec<T>>,

    /// Temporal memory for each node
    temporal_memory: Vec<TemporalMemory<T>>,

    /// Time encoder (Fourier features)
    time_encoder: FourierTimeEncoder<T>,

    /// Aggregation strategy for temporal events
    aggregator: TemporalAggregator<T>,

    /// Current logical time
    current_time: f64,

    /// Configuration
    config: TemporalConfig,
}

/// Configuration for temporal GNN
#[derive(Clone)]
pub struct TemporalConfig {
    /// Embedding dimension
    pub dimension: usize,

    /// Number of Fourier frequencies for time encoding
    pub num_frequencies: usize,

    /// Memory decay rate (exponential decay)
    pub decay_rate: f32,

    /// Maximum events to store per node
    pub max_events: usize,

    /// Time window for attention (seconds)
    pub attention_window: f64,

    /// Update strategy
    pub update_strategy: UpdateStrategy,
}

/// Temporal memory for a single node
pub struct TemporalMemory<T: Float> {
    /// Ring buffer of recent events
    events: RingBuffer<TemporalEvent<T>>,

    /// Cached aggregated embedding
    aggregated: Option<Vec<T>>,

    /// Last update timestamp
    last_update: f64,

    /// Decay rate for exponential decay
    decay_rate: f32,

    /// Dirty flag (needs re-aggregation)
    dirty: bool,
}

/// Single temporal event (interaction, update, etc.)
#[derive(Clone, Debug)]
pub struct TemporalEvent<T: Float> {
    /// Event timestamp
    timestamp: f64,

    /// Event value (delta embedding or full value)
    value: Vec<T>,

    /// Event weight/importance
    weight: f32,

    /// Event type
    event_type: EventType,
}

/// Type of temporal event
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EventType {
    /// Full embedding update
    FullUpdate,

    /// Delta (add to current embedding)
    Delta,

    /// Interaction with another node
    Interaction { neighbor_id: usize },

    /// External signal (click, purchase, etc.)
    ExternalSignal,
}

/// Fourier time encoding for continuous time
pub struct FourierTimeEncoder<T: Float> {
    /// Frequencies for sin/cos encoding
    /// f_i = 2π / (base_period * 2^i)
    frequencies: Vec<f64>,

    /// Output dimension (2 * num_frequencies)
    dimension: usize,

    /// Base period (e.g., 86400 for daily periodicity)
    base_period: f64,
}

impl<T: Float> FourierTimeEncoder<T> {
    /// Encode timestamp as Fourier features
    /// encoding(t) = [sin(f_1*t), cos(f_1*t), sin(f_2*t), cos(f_2*t), ...]
    pub fn encode(&self, timestamp: f64) -> Vec<T>;

    /// Create with default frequencies (hourly to yearly)
    pub fn new_default(num_frequencies: usize) -> Self;
}

/// Temporal aggregation strategies
pub enum TemporalAggregator<T: Float> {
    /// Exponential decay: w_i = exp(-λ * (t_now - t_i))
    ExponentialDecay { decay_rate: f32 },

    /// Time-windowed average (events within window)
    WindowedAverage { window_size: f64 },

    /// Attention-based aggregation
    Attention { attention_fn: Box<dyn Fn(f64, f64) -> f32 + Send + Sync> },

    /// Latest value only (no aggregation)
    Latest,
}

/// Update strategy for temporal embeddings
#[derive(Clone, Copy, Debug)]
pub enum UpdateStrategy {
    /// Eager: Update aggregated embedding immediately
    Eager,

    /// Lazy: Update only when queried
    Lazy,

    /// Batch: Update in batches every N events
    Batch { batch_size: usize },
}

/// Temporal HNSW index supporting time-aware queries
pub struct TemporalHNSW<T: Float> {
    /// Temporal GNN for computing embeddings
    temporal_gnn: TemporalGNN<T>,

    /// Time-sliced HNSW indexes (for efficient time-range queries)
    time_slices: BTreeMap<TimeRange, HNSWIndex>,

    /// Active index (current time)
    active_index: HNSWIndex,

    /// Slice configuration
    slice_config: SliceConfig,
}

/// Time range for index slicing
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeRange {
    start: u64,  // Unix timestamp
    end: u64,
}

/// Configuration for time slicing
#[derive(Clone)]
pub struct SliceConfig {
    /// Slice duration (seconds)
    slice_duration: u64,

    /// Number of historical slices to maintain
    max_slices: usize,

    /// Re-index strategy when slice is full
    reindex_strategy: ReindexStrategy,
}

/// Re-indexing strategy
#[derive(Clone, Copy, Debug)]
pub enum ReindexStrategy {
    /// Create new slice, archive old
    Slide,

    /// Merge old slices
    Merge,

    /// Rebuild from scratch
    Rebuild,
}

/// Result with temporal score
#[derive(Clone, Debug)]
pub struct TemporalSearchResult {
    /// Node ID
    pub id: usize,

    /// Spatial distance (embedding similarity)
    pub distance: f32,

    /// Temporal score (recency, relevance)
    pub temporal_score: f32,

    /// Combined score
    pub combined_score: f32,

    /// Timestamp of most recent event
    pub last_update: f64,
}
```

### Key Algorithms

#### Algorithm 1: Temporal Embedding Computation

```pseudocode
function compute_embedding_at_time(
    node_id: usize,
    t: f64,
    gnn: &TemporalGNN,
) -> Vec<T>:
    // Get base embedding and temporal memory
    base = gnn.base_embeddings[node_id]
    memory = gnn.temporal_memory[node_id]

    // Check cache
    if !memory.dirty && memory.last_update >= t:
        return memory.aggregated.clone()

    // Aggregate temporal events with decay
    temporal_component = Vec::zeros(base.len())
    total_weight = 0.0

    for event in memory.events:
        if event.timestamp > t:
            continue  // Future event, skip

        // Compute decay weight
        time_delta = t - event.timestamp
        decay_weight = exp(-gnn.config.decay_rate * time_delta)
        effective_weight = event.weight * decay_weight

        // Aggregate based on event type
        match event.event_type:
            FullUpdate:
                // Use event value directly (with decay)
                temporal_component = event.value * effective_weight
                total_weight = effective_weight
                break  // Full update overrides previous

            Delta:
                // Add delta to accumulator
                temporal_component += event.value * effective_weight
                total_weight += effective_weight

            Interaction { neighbor_id }:
                // Get neighbor embedding (recursive)
                neighbor_emb = compute_embedding_at_time(neighbor_id, t, gnn)
                temporal_component += neighbor_emb * effective_weight
                total_weight += effective_weight

    // Normalize and combine with base
    if total_weight > 0.0:
        temporal_component /= total_weight
        alpha = 0.7  // Blend ratio (tunable)
        result = alpha * base + (1 - alpha) * temporal_component
    else:
        result = base  // No events, use base

    // Add time encoding
    time_encoding = gnn.time_encoder.encode(t)
    result = concat(result, time_encoding)

    // Cache result
    memory.aggregated = Some(result.clone())
    memory.last_update = t
    memory.dirty = false

    return result
```

#### Algorithm 2: Fourier Time Encoding

```pseudocode
function encode_time_fourier(t: f64, encoder: &FourierTimeEncoder) -> Vec<T>:
    // Normalize timestamp to [0, 1] range based on base period
    t_normalized = (t % encoder.base_period) / encoder.base_period

    encoding = Vec::new()

    for freq in encoder.frequencies:
        // Compute sin and cos features
        angle = 2.0 * PI * freq * t_normalized
        encoding.push(sin(angle))
        encoding.push(cos(angle))

    return encoding

function create_frequency_schedule(num_frequencies: usize, base_period: f64) -> Vec<f64>:
    // Create exponentially spaced frequencies
    // Captures patterns from hours to years
    frequencies = Vec::new()

    for i in 0..num_frequencies:
        // Frequency decreases exponentially: f_i = 1 / (base_period * 2^i)
        freq = 1.0 / (base_period * 2.0.powi(i))
        frequencies.push(freq)

    return frequencies

    // Example with base_period = 86400 (1 day):
    // f_0 = 1/86400      (daily)
    // f_1 = 1/172800     (2-day)
    // f_2 = 1/345600     (4-day / weekly)
    // f_3 = 1/691200     (8-day / bi-weekly)
    // ...
    // f_8 = 1/22118400   (256-day / yearly)
```

#### Algorithm 3: Temporal Attention Aggregation

```pseudocode
function aggregate_events_with_attention(
    events: &[TemporalEvent],
    query_time: f64,
    config: &TemporalConfig,
) -> Vec<T>:
    if events.is_empty():
        return Vec::zeros(config.dimension)

    // Compute attention weights for each event
    attention_weights = Vec::new()

    for event in events:
        time_delta = query_time - event.timestamp

        // Temporal attention: closer events get higher weight
        // w(t) = exp(-(t_delta / window)²) * event.weight
        normalized_delta = time_delta / config.attention_window
        temporal_attention = exp(-normalized_delta * normalized_delta)

        weight = temporal_attention * event.weight
        attention_weights.push(weight)

    // Normalize weights (softmax)
    total_weight = attention_weights.sum()
    if total_weight > 0.0:
        attention_weights = attention_weights.map(|w| w / total_weight)
    else:
        return Vec::zeros(config.dimension)

    // Weighted sum of event values
    aggregated = Vec::zeros(config.dimension)
    for (event, weight) in zip(events, attention_weights):
        aggregated += event.value * weight

    return aggregated
```

#### Algorithm 4: Incremental Update

```pseudocode
function update_embedding_incremental(
    node_id: usize,
    event: TemporalEvent,
    gnn: &mut TemporalGNN,
    index: &mut TemporalHNSW,
) -> Result<()>:
    // Add event to temporal memory
    memory = &mut gnn.temporal_memory[node_id]
    memory.events.push(event)
    memory.dirty = true

    // Update current time
    gnn.current_time = max(gnn.current_time, event.timestamp)

    // Update strategy determines when to recompute
    match gnn.config.update_strategy:
        Eager:
            // Recompute embedding immediately
            new_embedding = compute_embedding_at_time(
                node_id,
                gnn.current_time,
                gnn
            )

            // Update HNSW index
            index.update_vector(node_id, new_embedding)?

        Lazy:
            // Mark as dirty, update on next query
            // (already done above)

        Batch { batch_size }:
            memory.pending_updates += 1
            if memory.pending_updates >= batch_size:
                // Trigger batch update
                batch_update_embeddings(gnn, index)?
                memory.pending_updates = 0

    // Decay old events if buffer is full
    if memory.events.len() > gnn.config.max_events:
        memory.events.remove_oldest()

    // Check if we need to create new time slice
    current_slice = index.time_slices.last_entry().unwrap()
    if current_slice.end < event.timestamp:
        create_new_time_slice(index)?

    Ok(())
```

#### Algorithm 5: Time-Range Search

```pseudocode
function search_time_range(
    query: &[T],
    t_start: f64,
    t_end: f64,
    k: usize,
    index: &TemporalHNSW,
) -> Vec<TemporalSearchResult>:
    // Find relevant time slices
    relevant_slices = index.time_slices
        .range(TimeRange { start: t_start, end: t_end })
        .collect()

    // Search each slice
    all_results = Vec::new()

    for (time_range, slice_index) in relevant_slices:
        // Compute query embedding at midpoint of slice
        t_query = (time_range.start + time_range.end) / 2.0
        query_temporal = index.temporal_gnn.compute_embedding_at_time(
            QUERY_ID,  // Special query node
            t_query,
        )

        // Search slice
        slice_results = slice_index.search(&query_temporal, k * 2)

        // Add temporal scores
        for result in slice_results:
            // Spatial score (embedding similarity)
            spatial_score = 1.0 - result.distance

            // Temporal score (recency within range)
            node_time = index.temporal_gnn.temporal_memory[result.id].last_update
            recency = 1.0 - (t_end - node_time) / (t_end - t_start)
            temporal_score = recency

            // Combined score (weighted)
            combined_score = 0.7 * spatial_score + 0.3 * temporal_score

            all_results.push(TemporalSearchResult {
                id: result.id,
                distance: result.distance,
                temporal_score,
                combined_score,
                last_update: node_time,
            })

    // Merge and re-rank by combined score
    all_results.sort_by(|a, b| b.combined_score.cmp(&a.combined_score))
    all_results.dedup_by_key(|r| r.id)  // Remove duplicates
    all_results.truncate(k)

    return all_results
```

### API Design

```rust
// Public API
pub mod temporal {
    use super::*;

    /// Create temporal GNN with configuration
    pub fn create_temporal_gnn<T: Float>(
        base_embeddings: Vec<Vec<T>>,
        config: TemporalConfig,
    ) -> Result<TemporalGNN<T>, Error>;

    /// Update node embedding with event
    pub fn update_node<T: Float>(
        gnn: &mut TemporalGNN<T>,
        node_id: usize,
        event: TemporalEvent<T>,
    ) -> Result<(), Error>;

    /// Compute embedding at specific time
    pub fn get_embedding_at_time<T: Float>(
        gnn: &TemporalGNN<T>,
        node_id: usize,
        timestamp: f64,
    ) -> Vec<T>;

    /// Build temporal HNSW index
    pub fn build_temporal_index<T: Float>(
        gnn: TemporalGNN<T>,
        hnsw_params: HNSWParams,
        slice_config: SliceConfig,
    ) -> Result<TemporalHNSW<T>, Error>;

    /// Search at specific time
    pub fn search_at_time<T: Float>(
        index: &TemporalHNSW<T>,
        query: &[T],
        timestamp: f64,
        k: usize,
    ) -> Vec<TemporalSearchResult>;

    /// Search within time range
    pub fn search_time_range<T: Float>(
        index: &TemporalHNSW<T>,
        query: &[T],
        t_start: f64,
        t_end: f64,
        k: usize,
    ) -> Vec<TemporalSearchResult>;
}

// Advanced API
pub mod temporal_advanced {
    /// Create custom time encoder
    pub fn create_time_encoder<T: Float>(
        frequencies: Vec<f64>,
        base_period: f64,
    ) -> FourierTimeEncoder<T>;

    /// Custom aggregation function
    pub fn set_custom_aggregator<T: Float>(
        gnn: &mut TemporalGNN<T>,
        aggregator: Box<dyn Fn(&[TemporalEvent<T>], f64) -> Vec<T>>,
    );

    /// Export temporal memory for analysis
    pub fn export_temporal_memory<T: Float>(
        gnn: &TemporalGNN<T>,
        node_id: usize,
    ) -> Vec<TemporalEvent<T>>;

    /// Trigger manual re-indexing
    pub fn reindex_temporal_hnsw<T: Float>(
        index: &mut TemporalHNSW<T>,
    ) -> Result<(), Error>;
}
```

## Integration Points

### Affected Crates/Modules

1. **ruvector-gnn** (Major Changes)
   - Add temporal memory to GNN layers
   - Implement time-aware message passing
   - Extend GNN forward pass with time parameter

2. **ruvector-hnsw** (Moderate Changes)
   - Support time-sliced indexes
   - Add temporal query methods
   - Implement incremental updates

3. **ruvector-core** (Minor Changes)
   - Add time encoding utilities
   - Extend embedding types with temporal metadata

4. **ruvector-gnn-node** (Moderate Changes)
   - Add TypeScript bindings for temporal queries
   - Expose streaming update API
   - Add time-range search to JavaScript API

### New Modules to Create

```
crates/ruvector-temporal/
├── src/
│   ├── lib.rs                          # Public API
│   ├── gnn/
│   │   ├── mod.rs                      # Temporal GNN
│   │   ├── memory.rs                   # Temporal memory
│   │   ├── aggregation.rs              # Event aggregation
│   │   └── update.rs                   # Incremental updates
│   ├── encoding/
│   │   ├── mod.rs                      # Time encoding
│   │   ├── fourier.rs                  # Fourier features
│   │   ├── learned.rs                  # Learned time embeddings
│   │   └── periodic.rs                 # Periodic encodings
│   ├── attention/
│   │   ├── mod.rs                      # Temporal attention
│   │   ├── weights.rs                  # Attention computation
│   │   └── decay.rs                    # Decay functions
│   ├── index/
│   │   ├── mod.rs                      # Temporal HNSW
│   │   ├── slicing.rs                  # Time-based slicing
│   │   ├── search.rs                   # Temporal search
│   │   └── maintenance.rs              # Index maintenance
│   ├── events/
│   │   ├── mod.rs                      # Event types
│   │   ├── buffer.rs                   # Ring buffer
│   │   └── serialization.rs            # Event persistence
│   └── utils/
│       ├── time.rs                     # Time utilities
│       └── stats.rs                    # Statistics
├── tests/
│   ├── gnn_tests.rs                    # Temporal GNN
│   ├── encoding_tests.rs               # Time encoding
│   ├── search_tests.rs                 # Temporal search
│   └── integration_tests.rs            # End-to-end
├── benches/
│   ├── update_bench.rs                 # Update performance
│   ├── search_bench.rs                 # Search performance
│   └── memory_bench.rs                 # Memory efficiency
└── Cargo.toml
```

### Dependencies on Other Features

- **Synergies**:
  - **Attention Mechanisms** (Existing): Temporal attention uses same attention framework
  - **Adaptive Precision** (Feature 5): Old time slices can use lower precision
  - **Hyperbolic Embeddings** (Feature 4): Hierarchies may evolve over time

- **Conflicts**:
  - Static embeddings cannot be mixed with temporal in same index

## Regression Prevention

### What Existing Functionality Could Break

1. **Static Embedding Assumptions**
   - Risk: Code assumes embeddings don't change
   - Impact: Cached distances become invalid

2. **HNSW Graph Stability**
   - Risk: Graph structure assumes stable embeddings
   - Impact: Neighbors may become outdated

3. **Serialization**
   - Risk: Temporal state is complex to serialize
   - Impact: Index persistence may fail

4. **Performance**
   - Risk: Embedding computation now requires time parameter
   - Impact: Latency increase for every query

### Test Cases to Prevent Regressions

```rust
#[cfg(test)]
mod regression_tests {
    use super::*;

    #[test]
    fn test_static_embeddings_preserved() {
        // With no events, temporal should match static
        let gnn = create_temporal_gnn(base_embeddings, config).unwrap();

        for node_id in 0..gnn.num_nodes() {
            let temporal_emb = get_embedding_at_time(&gnn, node_id, 0.0);
            let static_emb = &base_embeddings[node_id];

            assert_embeddings_close(&temporal_emb, static_emb, 1e-6);
        }
    }

    #[test]
    fn test_time_invariance_without_events() {
        // Querying at different times should give same result if no events
        let gnn = create_temporal_gnn(base_embeddings, config).unwrap();

        let emb_t0 = get_embedding_at_time(&gnn, node_id, 0.0);
        let emb_t1000 = get_embedding_at_time(&gnn, node_id, 1000.0);

        assert_embeddings_close(&emb_t0, &emb_t1000, 1e-6);
    }

    #[test]
    fn test_temporal_decay_monotonic() {
        // Influence should decrease monotonically with time
        let mut gnn = create_temporal_gnn(base_embeddings, config).unwrap();

        // Add event at t=0
        update_node(&mut gnn, node_id, event_at_time(0.0)).unwrap();

        let emb_t1 = get_embedding_at_time(&gnn, node_id, 1.0);
        let emb_t10 = get_embedding_at_time(&gnn, node_id, 10.0);
        let emb_t100 = get_embedding_at_time(&gnn, node_id, 100.0);

        let dist_1 = distance(&emb_t1, &base_embeddings[node_id]);
        let dist_10 = distance(&emb_t10, &base_embeddings[node_id]);
        let dist_100 = distance(&emb_t100, &base_embeddings[node_id]);

        // Embedding should converge back to base over time
        assert!(dist_1 > dist_10);
        assert!(dist_10 > dist_100);
    }

    #[test]
    fn test_search_consistency_across_time_slices() {
        // Searching at slice boundary should give consistent results
        let index = build_temporal_index(gnn, hnsw_params, slice_config).unwrap();

        let t_boundary = slice_config.slice_duration as f64;
        let results_before = search_at_time(&index, &query, t_boundary - 1.0, 10);
        let results_after = search_at_time(&index, &query, t_boundary + 1.0, 10);

        // Top results should be similar (allowing for some variation)
        let overlap = compute_overlap(&results_before, &results_after, 5);
        assert!(overlap >= 0.6, "Overlap {} < 0.6", overlap);
    }
}
```

### Backward Compatibility Strategy

1. **Optional Temporal Features**
   ```rust
   pub enum IndexType {
       Static(HNSWIndex),
       Temporal(TemporalHNSW),
   }

   // Unified API
   pub fn search(index: &IndexType, query: &[f32], k: usize) -> Vec<SearchResult> {
       match index {
           Static(idx) => idx.search(query, k),
           Temporal(idx) => idx.search_at_time(query, current_time(), k),
       }
   }
   ```

2. **Migration Path**
   ```rust
   pub fn convert_to_temporal(
       static_index: HNSWIndex,
       config: TemporalConfig,
   ) -> Result<TemporalHNSW, Error> {
       // Use static embeddings as base
       // Initialize empty temporal memory
       // Create single time slice
   }
   ```

3. **Feature Flag**
   ```toml
   [features]
   default = ["static-only"]
   temporal = ["dep:chrono", "dep:ring-buffer"]
   ```

## Implementation Phases

### Phase 1: Core Implementation (Weeks 1-2)

**Goal**: Implement temporal memory and time encoding

**Tasks**:
1. Create `ruvector-temporal` crate
2. Implement `TemporalMemory` with ring buffer
3. Implement `FourierTimeEncoder`
4. Add temporal event types
5. Implement exponential decay aggregation
6. Write unit tests

**Deliverables**:
- Working temporal memory
- Time encoding with Fourier features
- Event aggregation

**Success Criteria**:
- Time encoding captures periodic patterns
- Decay aggregation works correctly
- Memory overhead < 20% per node

### Phase 2: Integration (Weeks 3-4)

**Goal**: Integrate temporal GNN with HNSW

**Tasks**:
1. Implement `TemporalGNN`
2. Add time-sliced HNSW indexes
3. Implement temporal search
4. Add incremental update mechanism
5. Create migration from static indexes

**Deliverables**:
- Functioning `TemporalHNSW`
- Time-range search
- Incremental updates

**Success Criteria**:
- Search works across time slices
- Updates complete in < 1ms
- Accuracy matches static baseline

### Phase 3: Optimization (Weeks 5-6)

**Goal**: Optimize performance and scalability

**Tasks**:
1. Optimize embedding computation (caching)
2. Parallel time-slice search
3. Efficient event buffer management
4. Benchmark update throughput
5. Profile and optimize hotspots

**Deliverables**:
- Optimized temporal embedding computation
- Parallel search across slices
- Performance benchmarks

**Success Criteria**:
- Update throughput > 10k events/sec
- Search latency < 2x static baseline
- Memory overhead < 30%

### Phase 4: Production Hardening (Weeks 7-8)

**Goal**: Production-ready with monitoring and examples

**Tasks**:
1. Add comprehensive documentation
2. Create example applications:
   - Streaming recommendation
   - Temporal knowledge graph
3. Add monitoring/metrics
4. Performance tuning
5. Create deployment guide

**Deliverables**:
- API documentation
- Example applications
- Deployment guide
- Monitoring dashboards

**Success Criteria**:
- Documentation completeness > 90%
- Examples demonstrate temporal accuracy gains
- Zero P0/P1 bugs

## Success Metrics

### Performance Benchmarks

**Update Latency**:
- Single event update: < 100µs (lazy), < 1ms (eager)
- Batch update (100 events): < 5ms
- Full re-aggregation: < 10ms per node

**Search Latency**:
- Point-in-time search: < 2x static baseline
- Time-range search: < 3x static baseline
- Multi-slice search: Sub-linear in number of slices

**Throughput**:
- Event ingestion: > 10k events/sec
- Concurrent queries: > 1000 QPS

### Accuracy Metrics

**Temporal Prediction**:
- Next-item prediction: 15-25% improvement over static
- Trend detection: 80%+ accuracy
- Concept drift adaptation: < 5% accuracy loss

**Embedding Quality**:
- Time-aware cosine similarity: > 0.90 correlation with ground truth
- Temporal consistency: < 10% drift between adjacent time slices

### Memory/Latency Targets

**Memory Usage**:
- Temporal memory per node: < 1KB (100 events)
- Time encoding overhead: 5-10% of base embedding
- Total overhead: 20-30% vs. static

**Latency Breakdown**:
- Event aggregation: 40-50% of time
- Time encoding: 10-15% of time
- Base embedding: 30-40% of time
- Other: < 10% of time

## Risks and Mitigations

### Technical Risks

**Risk 1: Embedding Drift and Instability**
- **Severity**: High
- **Impact**: Embeddings change too rapidly, poor search quality
- **Probability**: Medium
- **Mitigation**:
  - Tune decay rate conservatively
  - Blend with static base embedding (alpha parameter)
  - Add stability constraints (max change per time unit)
  - Monitor drift metrics

**Risk 2: Time Slice Proliferation**
- **Severity**: Medium
- **Impact**: Memory explosion from too many slices
- **Probability**: High
- **Mitigation**:
  - Automatic slice merging
  - Configurable max slices with LRU eviction
  - Adaptive slicing based on update frequency
  - Compression of old slices

**Risk 3: Complex Temporal Queries**
- **Severity**: Medium
- **Impact**: Poor performance for time-range queries
- **Probability**: Medium
- **Mitigation**:
  - Index optimization (skip lists, interval trees)
  - Parallel slice search
  - Result caching
  - Query planning based on time range

**Risk 4: Event Ordering Issues**
- **Severity**: High
- **Impact**: Out-of-order events corrupt temporal state
- **Probability**: Medium
- **Mitigation**:
  - Timestamp validation on insert
  - Out-of-order buffer with re-sorting
  - Eventual consistency model
  - Version vectors for distributed updates

**Risk 5: Time Encoding Ineffectiveness**
- **Severity**: Medium
- **Impact**: Fourier features don't capture patterns
- **Probability**: Low
- **Mitigation**:
  - Learned time embeddings (alternative)
  - Adaptive frequency selection
  - Domain-specific encodings
  - Hybrid encodings (Fourier + learned)

**Risk 6: Serialization Complexity**
- **Severity**: Medium
- **Impact**: Difficult to save/restore temporal state
- **Probability**: High
- **Mitigation**:
  - Incremental serialization (event log)
  - Snapshot + event replay architecture
  - Compression of event history
  - Clear versioning scheme

### Mitigation Summary Table

| Risk | Mitigation Strategy | Owner | Timeline |
|------|-------------------|-------|----------|
| Embedding drift | Decay tuning + stability constraints | Research team | Phase 1-2 |
| Slice proliferation | Auto-merge + LRU eviction | Core team | Phase 2 |
| Query performance | Parallel search + caching | Perf team | Phase 3 |
| Event ordering | Validation + out-of-order buffer | Core team | Phase 1 |
| Encoding ineffectiveness | Learned embeddings fallback | Research team | Post-v1 |
| Serialization complexity | Event log architecture | Infrastructure | Phase 2 |

---

## References

1. **Xu et al. (2020)**: "Inductive Representation Learning on Temporal Graphs"
2. **Rossi et al. (2020)**: "Temporal Graph Networks for Deep Learning on Dynamic Graphs"
3. **Kazemi et al. (2020)**: "Representation Learning for Dynamic Graphs: A Survey"
4. **Vaswani et al. (2017)**: "Attention is All You Need" (Positional encoding)
5. **Tancik et al. (2020)**: "Fourier Features Let Networks Learn High Frequency Functions"

## Appendix: Time Encoding Details

### Fourier Time Encoding Formula

For timestamp `t` and frequency set `{f_1, ..., f_k}`:

```
φ(t) = [sin(2πf_1t), cos(2πf_1t), sin(2πf_2t), cos(2πf_2t), ..., sin(2πf_kt), cos(2πf_kt)]
```

### Default Frequency Schedule

Base period: 86400 seconds (1 day)

| Index | Frequency (Hz) | Period (days) | Captures |
|-------|---------------|---------------|----------|
| 0 | 1.157e-5 | 1 | Daily patterns |
| 1 | 5.787e-6 | 2 | Bi-daily |
| 2 | 2.894e-6 | 4 | 4-day cycle |
| 3 | 1.447e-6 | 8 | Weekly |
| 4 | 7.234e-7 | 16 | Bi-weekly |
| 5 | 3.617e-7 | 32 | Monthly |
| 6 | 1.809e-7 | 64 | Bi-monthly |
| 7 | 9.043e-8 | 128 | Quarterly |
| 8 | 4.521e-8 | 256 | Yearly |

### Exponential Decay Formula

Weight for event at time `t_i` when querying at `t_now`:

```
w(t_i, t_now) = exp(-λ * (t_now - t_i))
```

Typical decay rates:
- Fast: λ = 1.0 (half-life ≈ 0.7 time units)
- Medium: λ = 0.1 (half-life ≈ 7 time units)
- Slow: λ = 0.01 (half-life ≈ 70 time units)

Half-life calculation: `t_½ = ln(2) / λ ≈ 0.693 / λ`
