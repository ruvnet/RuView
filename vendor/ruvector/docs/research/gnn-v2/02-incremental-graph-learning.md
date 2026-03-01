# Incremental Graph Learning (ATLAS) - Implementation Plan

## Overview

### Problem Statement

Current GNN computation in ruvector is **full-graph recomputation**: whenever the graph changes (new vectors added, edges modified), the entire GNN must re-run forward passes over all nodes. This causes severe performance bottlenecks:

- **Slow Updates**: Adding 1,000 vectors to a 1M-node graph requires recomputing 1M+ node embeddings
- **Wasted Computation**: Most nodes are unaffected by localized changes
- **Poor Scalability**: O(N) update time where N = total graph size
- **Latency Spikes**: Updates block queries, causing P99 latency degradation
- **Memory Pressure**: Full-graph activations stored during backpropagation

Real-world impact:
- Vector insertion rate limited to ~100 vectors/second (vs 10,000+ for index-only updates)
- GNN updates take 10-100x longer than HNSW index updates
- Cannot support real-time streaming workloads

### Proposed Solution

**ATLAS (Adaptive Topology-Aware Learning Accelerator System)**: An incremental graph learning framework that updates only affected subgraphs:

1. **Dirty Node Tracking**: Mark nodes whose features/edges changed
2. **Dependency Propagation**: Compute k-hop affected region (receptive field)
3. **Incremental Forward Pass**: Recompute only dirty + affected nodes
4. **Activation Caching**: Reuse cached activations for unchanged nodes
5. **Lazy Materialization**: Defer updates to batch changes efficiently

**Key Insight**: Graph neural networks have bounded receptive fields. A k-layer GNN only needs information from k-hop neighbors. If a node's k-hop neighborhood is unchanged, its embedding is unchanged.

### Expected Benefits

**Quantified Performance Improvements:**

| Metric | Current (Full) | ATLAS (Incremental) | Improvement |
|--------|----------------|---------------------|-------------|
| Update Latency (1K vectors) | 500ms | 5ms | **100x faster** |
| Update Latency (10K vectors) | 5s | 50ms | **100x faster** |
| Throughput (vectors/sec) | 100 | 10,000 | **100x faster** |
| Memory (activation storage) | 1GB (full graph) | 10MB (dirty region) | **100x reduction** |
| Query Availability | Blocked during update | Concurrent | **Continuous** |

**Qualitative Benefits:**
- Real-time vector streaming support
- No query latency spikes during updates
- Memory-efficient updates
- Support for continuous learning workflows

## Technical Design

### Architecture Diagram (ASCII Art)

```
┌─────────────────────────────────────────────────────────────────┐
│                ATLAS Incremental Learning System                 │
└─────────────────────────────────────────────────────────────────┘

                    Vector Insert/Update/Delete
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Change Tracker                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  Dirty Node Set (BitSet)                                   │ │
│  │  - Nodes with changed features: [42, 137, 1025, ...]       │ │
│  │  - Nodes with changed edges: [43, 138, ...]                │ │
│  │  - Timestamps: last_modified[node_id] = timestamp          │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Dependency Analyzer                                             │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  Compute Affected Region (k-hop BFS)                       │ │
│  │                                                             │ │
│  │  dirty_nodes = {42, 137, 1025}                             │ │
│  │         │                                                   │ │
│  │         ▼                                                   │ │
│  │  1-hop neighbors: {41, 43, 136, 138, 1024, 1026}           │ │
│  │         │                                                   │ │
│  │         ▼                                                   │ │
│  │  2-hop neighbors: {40, 44, 135, 139, ...}                  │ │
│  │         │                                                   │ │
│  │         ▼ (repeat for k hops)                              │ │
│  │  affected_region = dirty ∪ 1-hop ∪ 2-hop ∪ ... ∪ k-hop    │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Activation Cache                                                │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  Cached Embeddings (per layer)                             │ │
│  │  ┌──────────────────────────────────────────────┐          │ │
│  │  │ Layer 0: {node_id → embedding, timestamp}    │          │ │
│  │  │   42 → [0.1, 0.3, ...] (STALE - dirty)       │          │ │
│  │  │   100 → [0.5, 0.2, ...] (FRESH - reuse!)     │          │ │
│  │  │   137 → [0.8, 0.1, ...] (STALE - affected)   │          │ │
│  │  └──────────────────────────────────────────────┘          │ │
│  │  ┌──────────────────────────────────────────────┐          │ │
│  │  │ Layer 1: {node_id → embedding, timestamp}    │          │ │
│  │  │   ...                                         │          │ │
│  │  └──────────────────────────────────────────────┘          │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Incremental Forward Pass                                        │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  For each layer l in GNN:                                  │ │
│  │    For each node in affected_region:                       │ │
│  │      if cached[l-1][node].is_fresh():                      │ │
│  │        embedding[l][node] = cached[l][node]  # Reuse!      │ │
│  │      else:                                                 │ │
│  │        # Recompute from previous layer                     │ │
│  │        neighbor_embeddings = [cached[l-1][n] for n in N(v)]│ │
│  │        embedding[l][node] = GNN_layer(neighbor_embeddings) │ │
│  │        cached[l][node] = embedding[l][node]  # Update cache│ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Batch Update Optimizer                                          │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  Lazy Materialization:                                     │ │
│  │  - Buffer changes until threshold (time/count)             │ │
│  │  - Coalesce dirty regions (merge overlapping k-hop sets)   │ │
│  │  - Sort affected nodes by layer propagation order          │ │
│  │  - Execute single batch update instead of N small updates  │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
                  Updated GNN Embeddings (partial)


┌─────────────────────────────────────────────────────────────────┐
│              Query Path (Concurrent with Updates)                │
└─────────────────────────────────────────────────────────────────┘

Query Request
     │
     ▼
┌─────────────────────────────────────────────────────────────────┐
│  Read-Write Lock (Activation Cache)                             │
│  - Queries acquire read lock (concurrent reads OK)              │
│  - Updates acquire write lock (blocks queries briefly)          │
│  - Most queries see slightly stale embeddings (acceptable)      │
└─────────────────────────────────────────────────────────────────┘
     │
     ▼
Retrieve embeddings from cache (mostly fresh)
     │
     ▼
Return query results
```

### Core Data Structures (Rust)

```rust
// File: crates/ruvector-gnn/src/incremental/mod.rs

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use bitvec::prelude::*;
use ndarray::Array2;

/// ATLAS incremental learning system
pub struct IncrementalGnn {
    /// Tracks which nodes have changed
    change_tracker: ChangeTracker,

    /// Caches computed activations per layer
    activation_cache: ActivationCache,

    /// Dependency graph for k-hop propagation
    dependency_graph: DependencyGraph,

    /// Batch update configuration
    batch_config: BatchUpdateConfig,

    /// Performance metrics
    metrics: IncrementalMetrics,

    /// GNN layer count (determines receptive field)
    num_layers: usize,
}

/// Tracks which nodes are dirty (need recomputation)
pub struct ChangeTracker {
    /// Dirty nodes (changed features or edges)
    dirty_nodes: BitVec,

    /// Timestamp of last modification per node
    last_modified: HashMap<u32, u64>,

    /// Global update counter
    update_counter: u64,

    /// Pending changes (buffered for batch processing)
    pending_changes: VecDeque<NodeChange>,
}

#[derive(Debug, Clone)]
pub enum NodeChange {
    /// Node features changed
    FeatureUpdate { node_id: u32, timestamp: u64 },

    /// Edges added/removed
    EdgeUpdate { node_id: u32, timestamp: u64 },

    /// Node deleted
    NodeDeleted { node_id: u32, timestamp: u64 },
}

/// Caches GNN activations (embeddings) per layer
pub struct ActivationCache {
    /// Cached embeddings per layer: layer_idx -> (node_id -> embedding)
    /// Wrapped in RwLock for concurrent read access during queries
    cache: Vec<Arc<RwLock<HashMap<u32, CachedActivation>>>>,

    /// Maximum cache size per layer (LRU eviction)
    max_size_per_layer: usize,

    /// Total cache hits/misses
    stats: CacheStats,
}

#[derive(Debug, Clone)]
pub struct CachedActivation {
    /// Node embedding for this layer
    pub embedding: Array2<f32>,

    /// Timestamp when computed
    pub timestamp: u64,

    /// Whether this activation is still valid
    pub is_valid: bool,
}

/// Computes affected regions for incremental updates
pub struct DependencyGraph {
    /// Graph structure for k-hop traversal
    graph: Arc<HnswGraph>,

    /// Precomputed k-hop neighborhoods (optional)
    khop_cache: HashMap<u32, Vec<HashSet<u32>>>,

    /// Number of GNN layers (k-hop receptive field)
    num_layers: usize,
}

/// Configuration for batch update optimization
#[derive(Debug, Clone)]
pub struct BatchUpdateConfig {
    /// Minimum changes to trigger batch update
    pub min_batch_size: usize,

    /// Maximum time to buffer changes (milliseconds)
    pub max_buffer_time_ms: u64,

    /// Whether to coalesce overlapping dirty regions
    pub coalesce_regions: bool,

    /// Whether to sort affected nodes topologically
    pub topological_sort: bool,
}

/// Performance metrics for incremental updates
#[derive(Debug, Default)]
pub struct IncrementalMetrics {
    /// Total incremental updates performed
    pub total_updates: u64,

    /// Average affected region size
    pub avg_affected_size: f64,

    /// Average update latency (microseconds)
    pub avg_update_latency_us: f64,

    /// Percentage of nodes recomputed (vs full graph)
    pub recompute_percentage: f64,

    /// Cache hit rate
    pub cache_hit_rate: f64,

    /// Time saved vs full recomputation
    pub time_saved_ratio: f64,
}

#[derive(Debug, Default)]
struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

/// Result of dependency analysis
#[derive(Debug)]
pub struct AffectedRegion {
    /// Nodes that need recomputation
    pub affected_nodes: HashSet<u32>,

    /// Organized by layer (for ordered processing)
    pub by_layer: Vec<Vec<u32>>,

    /// Estimated computation cost
    pub estimated_cost: usize,
}

/// Update plan for batch processing
pub struct UpdatePlan {
    /// Changes to apply
    pub changes: Vec<NodeChange>,

    /// Affected region
    pub affected_region: AffectedRegion,

    /// Execution order (topologically sorted)
    pub execution_order: Vec<u32>,

    /// Whether to invalidate cache entries
    pub invalidate_cache: bool,
}
```

### Key Algorithms (Pseudocode)

#### 1. Incremental GNN Update Algorithm

```python
function incremental_gnn_update(gnn: IncrementalGnn, changes: List[NodeChange]):
    """
    Update GNN embeddings incrementally based on changed nodes.

    Key idea: Only recompute nodes whose k-hop neighborhoods changed.
    """
    # Step 1: Mark dirty nodes
    dirty_nodes = set()
    for change in changes:
        dirty_nodes.add(change.node_id)
        gnn.change_tracker.mark_dirty(change.node_id)

    # Step 2: Compute affected region (k-hop propagation)
    affected_region = compute_affected_region(
        dirty_nodes,
        gnn.dependency_graph,
        k=gnn.num_layers
    )

    # Step 3: Invalidate cache for affected nodes
    for layer in range(gnn.num_layers):
        for node in affected_region.affected_nodes:
            gnn.activation_cache.invalidate(layer, node)

    # Step 4: Incremental forward pass (layer by layer)
    for layer in range(gnn.num_layers):
        # Get nodes to recompute at this layer
        nodes_to_compute = affected_region.by_layer[layer]

        for node in sorted(nodes_to_compute):  # Topological order
            # Check if we can reuse cached activation
            if gnn.activation_cache.is_valid(layer, node):
                continue  # Skip, already computed

            # Get neighbors from previous layer
            neighbors = gnn.dependency_graph.get_neighbors(node)
            neighbor_embeddings = []

            for neighbor in neighbors:
                # Try to reuse cached embedding from previous layer
                if layer == 0:
                    # Base features
                    emb = gnn.get_node_features(neighbor)
                else:
                    # Check cache first
                    cached = gnn.activation_cache.get(layer - 1, neighbor)
                    if cached is not None and cached.is_valid:
                        emb = cached.embedding  # Reuse!
                    else:
                        # Recursive recomputation (should not happen often)
                        emb = recompute_node(gnn, neighbor, layer - 1)

                neighbor_embeddings.append(emb)

            # Apply GNN layer (attention, aggregation, etc.)
            new_embedding = gnn.gnn_layers[layer].forward(
                node_features=gnn.get_node_features(node),
                neighbor_embeddings=neighbor_embeddings,
                edge_features=gnn.get_edge_features(node, neighbors)
            )

            # Update cache
            gnn.activation_cache.set(
                layer,
                node,
                CachedActivation(
                    embedding=new_embedding,
                    timestamp=gnn.change_tracker.update_counter,
                    is_valid=True
                )
            )

    # Step 5: Clear dirty flags
    gnn.change_tracker.clear_dirty(dirty_nodes)
    gnn.change_tracker.update_counter += 1

    # Step 6: Update metrics
    gnn.metrics.record_update(
        affected_size=len(affected_region.affected_nodes),
        total_nodes=gnn.dependency_graph.num_nodes()
    )


function compute_affected_region(dirty_nodes, graph, k):
    """
    Compute k-hop affected region via BFS.

    Returns nodes that need recomputation due to changed neighborhoods.
    """
    affected = set(dirty_nodes)
    current_frontier = set(dirty_nodes)

    # Propagate for k hops
    for hop in range(k):
        next_frontier = set()

        for node in current_frontier:
            # Get neighbors (reverse direction: who depends on this node?)
            # In GNN, node v depends on neighbors N(v), so we need reverse edges
            neighbors = graph.get_reverse_neighbors(node)

            for neighbor in neighbors:
                if neighbor not in affected:
                    affected.add(neighbor)
                    next_frontier.add(neighbor)

        current_frontier = next_frontier

        if not current_frontier:
            break  # No more propagation needed

    # Organize by layer for ordered processing
    by_layer = organize_by_layer(affected, graph, k)

    return AffectedRegion(
        affected_nodes=affected,
        by_layer=by_layer,
        estimated_cost=len(affected)
    )


function organize_by_layer(affected_nodes, graph, num_layers):
    """
    Organize affected nodes by layer for correct processing order.

    Layer 0 nodes must be computed before Layer 1, etc.
    """
    by_layer = [[] for _ in range(num_layers)]

    # Topological sort by dependency depth
    for node in affected_nodes:
        # Compute minimum layer where this node needs recomputation
        # (based on its position in the dependency graph)
        layer = compute_required_layer(node, graph, num_layers)
        by_layer[layer].append(node)

    return by_layer


function recompute_node(gnn, node, layer):
    """
    Recursively recompute a node's embedding at a given layer.

    This should be rare if cache is working properly.
    """
    if layer == 0:
        return gnn.get_node_features(node)

    # Get neighbors from previous layer
    neighbors = gnn.dependency_graph.get_neighbors(node)
    neighbor_embeddings = [
        recompute_node(gnn, neighbor, layer - 1)
        for neighbor in neighbors
    ]

    # Apply GNN layer
    embedding = gnn.gnn_layers[layer].forward(
        node_features=gnn.get_node_features(node),
        neighbor_embeddings=neighbor_embeddings,
        edge_features=gnn.get_edge_features(node, neighbors)
    )

    # Cache result
    gnn.activation_cache.set(layer, node, CachedActivation(
        embedding=embedding,
        timestamp=gnn.change_tracker.update_counter,
        is_valid=True
    ))

    return embedding
```

#### 2. Batch Update Optimization

```python
function batch_update_optimizer(gnn: IncrementalGnn):
    """
    Buffer and coalesce changes for efficient batch processing.

    Reduces overhead of many small updates.
    """
    buffer = gnn.change_tracker.pending_changes
    config = gnn.batch_config

    while True:
        # Wait for trigger condition
        if len(buffer) < config.min_batch_size:
            sleep_until(timeout=config.max_buffer_time_ms)

        if len(buffer) == 0:
            continue

        # Collect all pending changes
        changes = buffer.drain()

        # Coalesce overlapping dirty regions
        if config.coalesce_regions:
            changes = coalesce_changes(changes)

        # Create update plan
        plan = create_update_plan(gnn, changes)

        # Execute batch update
        execute_update_plan(gnn, plan)


function coalesce_changes(changes):
    """
    Merge overlapping changes to reduce redundant computation.

    Example: If node A changes at t=1 and t=5, only keep t=5.
    """
    # Deduplicate by node_id, keep latest timestamp
    latest_changes = {}
    for change in changes:
        node = change.node_id
        if node not in latest_changes or change.timestamp > latest_changes[node].timestamp:
            latest_changes[node] = change

    return list(latest_changes.values())


function create_update_plan(gnn, changes):
    """
    Create optimized execution plan for batch update.
    """
    # Compute affected region for all changes
    dirty_nodes = {change.node_id for change in changes}
    affected_region = compute_affected_region(
        dirty_nodes,
        gnn.dependency_graph,
        k=gnn.num_layers
    )

    # Topologically sort affected nodes for correct order
    if gnn.batch_config.topological_sort:
        execution_order = topological_sort(
            affected_region.affected_nodes,
            gnn.dependency_graph
        )
    else:
        execution_order = list(affected_region.affected_nodes)

    return UpdatePlan(
        changes=changes,
        affected_region=affected_region,
        execution_order=execution_order,
        invalidate_cache=True
    )


function execute_update_plan(gnn, plan):
    """
    Execute batch update with write lock on activation cache.
    """
    # Acquire write lock (blocks queries briefly)
    with gnn.activation_cache.write_lock():
        incremental_gnn_update(gnn, plan.changes)

    # Queries can resume with updated embeddings
```

#### 3. Concurrent Query Support

```python
function query_with_incremental_gnn(gnn, query_vector, k):
    """
    Query GNN embeddings while updates are happening.

    Uses read-write locks to allow concurrent reads.
    """
    # Acquire read lock (multiple queries can read concurrently)
    with gnn.activation_cache.read_lock():
        # Get embeddings from cache (might be slightly stale)
        embeddings = []
        for node_id in gnn.graph.all_nodes():
            # Try to get from cache
            cached = gnn.activation_cache.get(
                layer=gnn.num_layers - 1,  # Final layer
                node=node_id
            )

            if cached is not None and cached.is_valid:
                embeddings.append((node_id, cached.embedding))
            else:
                # Fallback: use base features (no GNN)
                base_features = gnn.get_node_features(node_id)
                embeddings.append((node_id, base_features))

        # Perform similarity search
        results = search_similar(query_vector, embeddings, k)

    return results
```

### API Design (Function Signatures)

```rust
// File: crates/ruvector-gnn/src/incremental/mod.rs

impl IncrementalGnn {
    /// Create a new incremental GNN system
    pub fn new(
        graph: Arc<HnswGraph>,
        num_layers: usize,
        batch_config: BatchUpdateConfig,
    ) -> Result<Self, GnnError>;

    /// Record a node feature update (triggers incremental recomputation)
    pub fn update_node_features(
        &mut self,
        node_id: u32,
        new_features: &[f32],
    ) -> Result<(), GnnError>;

    /// Record edge changes (triggers incremental recomputation)
    pub fn update_edges(
        &mut self,
        node_id: u32,
        added_edges: &[(u32, u32)],
        removed_edges: &[(u32, u32)],
    ) -> Result<(), GnnError>;

    /// Perform incremental update based on pending changes
    pub fn apply_incremental_update(&mut self) -> Result<UpdateStats, GnnError>;

    /// Force full graph recomputation (fallback)
    pub fn full_recompute(&mut self) -> Result<(), GnnError>;

    /// Get cached embedding for a node
    pub fn get_embedding(
        &self,
        node_id: u32,
        layer: usize,
    ) -> Option<Array2<f32>>;

    /// Check if cached embedding is valid
    pub fn is_embedding_valid(
        &self,
        node_id: u32,
        layer: usize,
    ) -> bool;

    /// Get incremental update metrics
    pub fn metrics(&self) -> &IncrementalMetrics;

    /// Clear all cached activations
    pub fn clear_cache(&mut self);
}

impl ChangeTracker {
    /// Mark a node as dirty (needs recomputation)
    pub fn mark_dirty(&mut self, node_id: u32);

    /// Check if a node is dirty
    pub fn is_dirty(&self, node_id: u32) -> bool;

    /// Clear dirty flag for a node
    pub fn clear_dirty(&mut self, node_id: u32);

    /// Get all dirty nodes
    pub fn get_dirty_nodes(&self) -> Vec<u32>;

    /// Buffer a change for batch processing
    pub fn buffer_change(&mut self, change: NodeChange);

    /// Drain all buffered changes
    pub fn drain_buffered(&mut self) -> Vec<NodeChange>;
}

impl ActivationCache {
    /// Create a new activation cache
    pub fn new(num_layers: usize, max_size_per_layer: usize) -> Self;

    /// Get cached activation
    pub fn get(&self, layer: usize, node_id: u32) -> Option<CachedActivation>;

    /// Set cached activation
    pub fn set(&mut self, layer: usize, node_id: u32, activation: CachedActivation);

    /// Invalidate cached activation
    pub fn invalidate(&mut self, layer: usize, node_id: u32);

    /// Check if activation is valid
    pub fn is_valid(&self, layer: usize, node_id: u32) -> bool;

    /// Acquire read lock (for concurrent queries)
    pub fn read_lock(&self) -> RwLockReadGuard<'_, HashMap<u32, CachedActivation>>;

    /// Acquire write lock (for updates)
    pub fn write_lock(&mut self) -> RwLockWriteGuard<'_, HashMap<u32, CachedActivation>>;

    /// Get cache statistics
    pub fn stats(&self) -> &CacheStats;

    /// Clear all cached activations
    pub fn clear(&mut self);
}

impl DependencyGraph {
    /// Create dependency graph from HNSW graph
    pub fn from_hnsw(graph: Arc<HnswGraph>, num_layers: usize) -> Self;

    /// Compute k-hop affected region from dirty nodes
    pub fn compute_affected_region(
        &self,
        dirty_nodes: &HashSet<u32>,
    ) -> AffectedRegion;

    /// Get reverse neighbors (who depends on this node?)
    pub fn get_reverse_neighbors(&self, node_id: u32) -> Vec<u32>;

    /// Precompute k-hop neighborhoods (optional optimization)
    pub fn precompute_khop_cache(&mut self) -> Result<(), GnnError>;
}

#[derive(Debug)]
pub struct UpdateStats {
    /// Number of nodes recomputed
    pub nodes_recomputed: usize,

    /// Total nodes in graph
    pub total_nodes: usize,

    /// Update latency (microseconds)
    pub latency_us: u64,

    /// Speedup vs full recomputation
    pub speedup_ratio: f64,
}
```

## Integration Points

### Affected Crates/Modules

1. **`ruvector-gnn`** (Primary)
   - New module: `src/incremental/mod.rs` - Core ATLAS system
   - New module: `src/incremental/change_tracker.rs` - Dirty node tracking
   - New module: `src/incremental/activation_cache.rs` - Embedding caching
   - New module: `src/incremental/dependency.rs` - Dependency analysis
   - Modified: `src/lib.rs` - Export incremental types

2. **`ruvector-core`** (Integration)
   - Modified: `src/index/hnsw.rs` - Notify GNN of graph changes
   - New: `src/index/hnsw_events.rs` - Event system for graph updates
   - Modified: `src/vector_store.rs` - Trigger incremental updates on insert/delete

3. **`ruvector-api`** (Configuration)
   - Modified: `src/config.rs` - Add incremental GNN config
   - Modified: `src/index_manager.rs` - Manage incremental update lifecycle

### New Modules to Create

```
crates/ruvector-gnn/
├── src/
│   ├── incremental/
│   │   ├── mod.rs                # Core IncrementalGnn
│   │   ├── change_tracker.rs     # ChangeTracker implementation
│   │   ├── activation_cache.rs   # ActivationCache implementation
│   │   ├── dependency.rs         # DependencyGraph implementation
│   │   ├── batch_optimizer.rs    # Batch update optimization
│   │   └── metrics.rs            # Performance tracking

crates/ruvector-core/
├── src/
│   ├── index/
│   │   ├── hnsw_events.rs        # Event system for graph changes

examples/
├── incremental_gnn/
│   ├── benchmark_updates.rs      # Benchmark incremental vs full
│   ├── streaming_workload.rs     # Real-time streaming example
│   └── README.md
```

### Dependencies on Other Features

**Depends On:**
- **GNN Layer Implementation (Issue #38)**: Needs working GNN layers to recompute embeddings
- **HNSW Index**: Needs graph structure for dependency analysis

**Synergies With:**
- **GNN-Guided Routing (Feature 1)**: Incremental updates keep routing model fresh
- **Neuro-Symbolic Query (Feature 3)**: Faster updates enable real-time constraint learning

**External Dependencies:**
- `bitvec` - Efficient BitSet for dirty node tracking
- `parking_lot` - RwLock for concurrent cache access
- `crossbeam` - Batch processing queue (optional)

## Regression Prevention

### What Existing Functionality Could Break

1. **GNN Embedding Correctness**
   - Risk: Incremental updates produce different embeddings than full recomputation
   - Impact: Incorrect query results, embedding drift

2. **Memory Leaks**
   - Risk: Activation cache grows unbounded if not evicted
   - Impact: OOM crashes

3. **Deadlocks**
   - Risk: Read-write lock contention between queries and updates
   - Impact: System hangs

4. **Stale Embeddings**
   - Risk: Cache invalidation logic misses affected nodes
   - Impact: Queries use outdated embeddings

5. **Update Ordering**
   - Risk: Concurrent updates applied in wrong order
   - Impact: Inconsistent graph state

### Test Cases to Prevent Regressions

```rust
// File: crates/ruvector-gnn/tests/incremental_regression_tests.rs

#[test]
fn test_incremental_matches_full_recomputation() {
    // Incremental updates must produce identical embeddings to full recompute
    let graph = build_test_graph(1000);
    let gnn_full = FullGnn::new(&graph, num_layers=3);
    let gnn_inc = IncrementalGnn::new(&graph, num_layers=3);

    // Apply 100 random updates
    let updates = generate_random_updates(100);

    // Full recomputation
    for update in &updates {
        apply_update_full(&mut gnn_full, update);
    }
    gnn_full.recompute_all();

    // Incremental updates
    for update in &updates {
        apply_update_incremental(&mut gnn_inc, update);
    }
    gnn_inc.apply_incremental_update();

    // Compare embeddings (should be identical within floating-point tolerance)
    for node_id in 0..1000 {
        let emb_full = gnn_full.get_embedding(node_id, layer=2);
        let emb_inc = gnn_inc.get_embedding(node_id, layer=2).unwrap();

        assert_embeddings_equal(&emb_full, &emb_inc, tolerance=1e-5);
    }
}

#[test]
fn test_cache_invalidation_correctness() {
    // All affected nodes must have cache invalidated
    let graph = build_test_graph(1000);
    let mut gnn = IncrementalGnn::new(&graph, num_layers=3);

    // Mark node 42 as dirty
    gnn.update_node_features(42, &random_features());

    // Compute affected region (3-hop)
    let affected = gnn.dependency_graph.compute_affected_region(&hashset!{42});

    // Check cache invalidation
    for node in &affected.affected_nodes {
        for layer in 0..3 {
            assert!(!gnn.activation_cache.is_valid(layer, *node),
                "Node {} layer {} should be invalidated", node, layer);
        }
    }
}

#[test]
fn test_incremental_speedup() {
    // Incremental updates must be ≥10x faster than full recompute
    let graph = build_test_graph(100_000);
    let mut gnn_full = FullGnn::new(&graph, num_layers=3);
    let mut gnn_inc = IncrementalGnn::new(&graph, num_layers=3);

    // Small update (100 nodes)
    let updates = generate_random_updates(100);

    // Benchmark full recomputation
    let start = Instant::now();
    for update in &updates {
        apply_update_full(&mut gnn_full, update);
    }
    gnn_full.recompute_all();
    let full_time = start.elapsed();

    // Benchmark incremental
    let start = Instant::now();
    for update in &updates {
        apply_update_incremental(&mut gnn_inc, update);
    }
    gnn_inc.apply_incremental_update();
    let inc_time = start.elapsed();

    let speedup = full_time.as_secs_f64() / inc_time.as_secs_f64();
    assert!(speedup >= 10.0, "Speedup: {:.1}x, expected ≥10x", speedup);
}

#[test]
fn test_concurrent_query_update() {
    // Queries must not block on updates (concurrent reads)
    let graph = Arc::new(build_test_graph(10_000));
    let gnn = Arc::new(RwLock::new(IncrementalGnn::new(&graph, num_layers=3)));

    // Spawn update thread
    let gnn_update = Arc::clone(&gnn);
    let update_handle = thread::spawn(move || {
        loop {
            let mut g = gnn_update.write().unwrap();
            g.update_node_features(rand::random(), &random_features());
            g.apply_incremental_update().unwrap();
            drop(g);  // Release lock
            sleep(Duration::from_millis(10));
        }
    });

    // Spawn query threads
    let query_handles: Vec<_> = (0..8)
        .map(|_| {
            let gnn_query = Arc::clone(&gnn);
            thread::spawn(move || {
                for _ in 0..1000 {
                    let g = gnn_query.read().unwrap();
                    let emb = g.get_embedding(rand::random::<u32>() % 10_000, layer=2);
                    assert!(emb.is_some());
                    drop(g);  // Release lock
                }
            })
        })
        .collect();

    // Wait for queries to complete
    for handle in query_handles {
        handle.join().unwrap();
    }

    // Should complete without deadlocks
}

#[test]
fn test_cache_memory_bounded() {
    // Cache must not exceed configured size limit
    let graph = build_test_graph(100_000);
    let mut gnn = IncrementalGnn::new(&graph, num_layers=3);

    // Configure small cache (1000 entries per layer)
    gnn.activation_cache = ActivationCache::new(3, max_size_per_layer=1000);

    // Perform many updates (should trigger evictions)
    for _ in 0..10_000 {
        gnn.update_node_features(rand::random(), &random_features());
        gnn.apply_incremental_update().unwrap();
    }

    // Check cache size
    for layer in 0..3 {
        let cache_size = gnn.activation_cache.layer_size(layer);
        assert!(cache_size <= 1000, "Layer {} cache size: {}, expected ≤1000", layer, cache_size);
    }
}
```

### Backward Compatibility Strategy

1. **Default Disabled**
   - Incremental GNN is opt-in via configuration
   - Existing code defaults to full recomputation

2. **Graceful Fallback**
   - If incremental update fails, fallback to full recompute
   - Log warning but do not crash

3. **Configuration Schema**
   ```yaml
   gnn:
     incremental:
       enabled: false  # Default: disabled
       batch_size: 100
       max_buffer_time_ms: 1000
       cache_size_per_layer: 10000
   ```

4. **API Compatibility**
   - Existing `Gnn::recompute()` still works (full recompute)
   - New `Gnn::incremental_update()` method added

## Implementation Phases

### Phase 1: Core Infrastructure (Week 1-2)

**Goal**: Working change tracking and activation cache

**Tasks**:
1. Implement `ChangeTracker` with BitSet
2. Implement `ActivationCache` with RwLock
3. Add unit tests for both
4. Benchmark cache performance (hit rate, contention)

**Deliverables**:
- `incremental/change_tracker.rs`
- `incremental/activation_cache.rs`
- Passing unit tests
- Benchmark report

**Success Criteria**:
- Change tracking overhead <1% of update time
- Cache hit rate >90% for typical workloads
- No deadlocks in concurrent access

### Phase 2: Dependency Analysis (Week 2-3)

**Goal**: Compute affected regions correctly

**Tasks**:
1. Implement `DependencyGraph` with k-hop BFS
2. Add topological sorting for update order
3. Test affected region computation on various graph topologies
4. Optimize with k-hop caching (optional)

**Deliverables**:
- `incremental/dependency.rs`
- Tests for k-hop propagation
- Performance benchmarks

**Success Criteria**:
- Affected region computation <10ms for 1K dirty nodes
- Correct propagation (matches ground truth)
- Handles edge cases (disconnected components, cycles)

### Phase 3: Incremental Forward Pass (Week 3-4)

**Goal**: Recompute only affected nodes

**Tasks**:
1. Implement incremental forward pass algorithm
2. Integrate with existing GNN layers
3. Add cache reuse logic
4. Test correctness vs full recomputation
5. Benchmark speedup

**Deliverables**:
- `incremental/mod.rs` (core algorithm)
- Correctness tests
- Performance benchmarks

**Success Criteria**:
- Embeddings match full recomputation (within tolerance)
- ≥10x speedup for small updates (<1% of graph)
- ≥100x speedup for tiny updates (<0.1% of graph)

### Phase 4: Batch Optimization (Week 4-5)

**Goal**: Efficient batch processing of updates

**Tasks**:
1. Implement batch update optimizer
2. Add change coalescing logic
3. Tune buffer size and timeout
4. Benchmark throughput improvement

**Deliverables**:
- `incremental/batch_optimizer.rs`
- Batch processing benchmarks
- Configuration guide

**Success Criteria**:
- Batch updates 2-5x faster than individual updates
- Latency <50ms for 1K batched changes
- No excessive buffering delays

### Phase 5: Production Hardening (Week 5-6)

**Goal**: Production-ready with safety guarantees

**Tasks**:
1. Add comprehensive error handling
2. Implement fallback to full recompute on errors
3. Add telemetry and observability
4. Write documentation
5. Stress testing (10M+ nodes, concurrent workloads)

**Deliverables**:
- Full error handling
- Regression test suite
- User documentation
- Performance report

**Success Criteria**:
- Zero crashes in stress tests
- Graceful degradation on errors
- Documentation complete

## Success Metrics

### Performance Benchmarks

**Primary Metrics** (Must Achieve):

| Workload | Current (Full) | Target (ATLAS) | Improvement |
|----------|----------------|----------------|-------------|
| 100 vector updates | 50ms | 0.5ms | **100x** |
| 1,000 vector updates | 500ms | 5ms | **100x** |
| 10,000 vector updates | 5s | 50ms | **100x** |
| Continuous stream (1K/s) | Blocked | 1K/s sustained | **∞** |

**Secondary Metrics**:

| Metric | Target |
|--------|--------|
| Cache hit rate | >90% |
| Memory overhead | <10% of base GNN |
| Concurrent query throughput | No degradation |
| Affected region ratio | <5% of graph (for 0.1% dirty nodes) |

### Accuracy Metrics

**Embedding Correctness**:
- Incremental embeddings must match full recomputation within `1e-5` tolerance (floating-point)
- Zero embedding drift over 1M updates

**Cache Invalidation**:
- 100% of affected nodes have cache invalidated (no stale embeddings used)
- Zero false negatives (missed invalidations)

### Memory/Latency Targets

**Memory**:
- Activation cache: <100MB per 1M nodes
- Change tracker: <10MB per 1M nodes (BitSet)
- Total overhead: <10% of base GNN memory

**Latency**:
- Update latency (100 vectors): <1ms
- Update latency (1K vectors): <10ms
- Update latency (10K vectors): <100ms
- Query latency: No increase (concurrent reads)

**Throughput**:
- Sustained update rate: 10,000 vectors/second
- Batch update throughput: 100,000 vectors/second

## Risks and Mitigations

### Technical Risks

**Risk 1: Cache Invalidation Bugs**

*Probability: High | Impact: Critical*

**Description**: Missing cache invalidations could cause stale embeddings to be used, leading to incorrect query results.

**Mitigation**:
- Extensive testing with known ground truth
- Add assertion checks in debug builds (compare incremental vs full)
- Implement cache consistency validation tool
- Conservative invalidation (over-invalidate rather than under-invalidate)
- Monitor embedding drift metrics in production

**Contingency**: Add "full recompute verification" mode that periodically checks incremental results against full recompute.

---

**Risk 2: Concurrency Bugs (Deadlocks, Race Conditions)**

*Probability: Medium | Impact: High*

**Description**: RwLock usage could introduce deadlocks or race conditions between queries and updates.

**Mitigation**:
- Use proven lock-free data structures where possible
- Lock ordering discipline (always acquire in same order)
- Timeout on lock acquisition
- Extensive concurrency testing with ThreadSanitizer
- Use parking_lot for better performance and diagnostics

**Contingency**: Fallback to single-threaded updates if concurrency issues arise.

---

**Risk 3: Memory Leak from Unbounded Cache**

*Probability: Medium | Impact: Medium*

**Description**: Activation cache could grow unbounded if eviction policy fails.

**Mitigation**:
- Implement strict LRU eviction
- Set hard memory limits with monitoring
- Add memory pressure detection
- Test with long-running workloads
- Provide cache clear API for manual intervention

**Contingency**: Add periodic cache clearing (e.g., every 1M updates) as safety net.

---

**Risk 4: k-Hop Propagation Overhead**

*Probability: Low | Impact: Medium*

**Description**: Computing k-hop affected regions could be slow on dense graphs.

**Mitigation**:
- Precompute k-hop neighborhoods (optional)
- Use approximate k-hop (prune low-degree nodes)
- Parallelize BFS traversal
- Cache affected regions for repeated patterns
- Profile and optimize hot paths

**Contingency**: Add configurable k-hop limit (user can reduce k if needed).

---

**Risk 5: Divergence from Full Recomputation**

*Probability: Low | Impact: High*

**Description**: Incremental updates could accumulate numerical errors, causing embedding drift over time.

**Mitigation**:
- Use same floating-point precision as full recompute
- Periodically run full recomputation to reset (e.g., daily)
- Monitor embedding distance metrics
- Add numerical stability tests
- Use higher precision (f64) for accumulation if needed

**Contingency**: Implement "full recompute every N updates" policy.

---

**Risk 6: Complex Debugging**

*Probability: High | Impact: Medium*

**Description**: Incremental update bugs are harder to debug than full recomputation.

**Mitigation**:
- Add extensive logging and telemetry
- Implement deterministic replay of update sequences
- Provide debugging tools (cache inspector, affected region visualizer)
- Add assertion modes for validation
- Document common failure modes

**Contingency**: Provide "debug mode" that runs both incremental and full in parallel for comparison.

---

### Summary Risk Matrix

| Risk | Probability | Impact | Mitigation Priority |
|------|-------------|--------|---------------------|
| Cache invalidation bugs | High | Critical | **CRITICAL** |
| Concurrency bugs | Medium | High | **HIGH** |
| Memory leak | Medium | Medium | HIGH |
| k-hop overhead | Low | Medium | Medium |
| Embedding divergence | Low | High | Medium |
| Complex debugging | High | Medium | LOW |

---

## Next Steps

1. **Prototype Phase 1**: Build change tracker and activation cache (1 week)
2. **Validate Approach**: Test on small graph (1K nodes), measure speedup (2 days)
3. **Scale Testing**: Test on realistic graph (100K nodes), identify bottlenecks (3 days)
4. **Integration**: Connect to HNSW index updates (1 week)
5. **Optimization**: Profile and optimize hot paths (ongoing)

**Key Decision Points**:
- After Phase 1: Is cache overhead acceptable? (<10% memory)
- After Phase 3: Does speedup meet targets? (≥10x required)
- After Phase 5: Are embeddings correct? (Pass all regression tests)

**Go/No-Go Criteria**:
- ✅ 10x+ speedup on small updates
- ✅ Zero embedding correctness regressions
- ✅ No concurrency bugs in stress tests
- ✅ Memory overhead <10%
