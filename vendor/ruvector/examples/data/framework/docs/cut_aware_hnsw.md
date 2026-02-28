# Cut-Aware HNSW: Dynamic Min-Cut Integration with Vector Search

## Overview

`cut_aware_hnsw.rs` implements a coherence-aware extension to HNSW (Hierarchical Navigable Small World) graphs that respects semantic boundaries in vector spaces. Traditional HNSW blindly follows similarity edges during search. Cut-aware HNSW adds "coherence gates" that halt expansion at weak cuts, keeping searches within semantically coherent regions.

## Architecture

### Core Components

1. **DynamicCutWatcher** - Tracks minimum cuts and graph coherence
   - Implements Stoer-Wagner algorithm for global min-cut
   - Incremental updates with caching for efficiency
   - Identifies boundary edges crossing partitions

2. **CutAwareHNSW** - Extended HNSW with coherence gating
   - Wraps standard HNSW index
   - Maintains cut watcher for edge weights
   - Supports both gated and ungated search modes

3. **CoherenceZone** - Regions of strong internal connectivity
   - Computed from min-cut partitions
   - Tracked with coherence ratios
   - Used for zone-aware queries

## Key Features

### 1. Coherence-Gated Search

```rust
let config = CutAwareConfig {
    coherence_gate_threshold: 0.3,  // Cuts below this are "weak"
    max_cross_cut_hops: 2,           // Max boundary crossings
    ..Default::default()
};

let mut index = CutAwareHNSW::new(config);

// Insert vectors
index.insert(node_id, &vector)?;

// Gated search (respects boundaries)
let gated_results = index.search_gated(&query, k);

// Ungated search (baseline)
let ungated_results = index.search_ungated(&query, k);
```

**Gated Search** will:
- Track cut crossings for each result
- Gate expansion at weak cuts (below threshold)
- Return coherence scores (1.0 = no cuts crossed)
- Prune expansions exceeding max_cross_cut_hops

### 2. Coherent Neighborhoods

Find all nodes reachable without crossing weak cuts:

```rust
let neighbors = index.coherent_neighborhood(node_id, radius);
// Returns nodes within `radius` hops that don't cross weak cuts
```

### 3. Zone-Based Queries

Partition the graph into coherence zones and query specific regions:

```rust
// Compute zones
let zones = index.compute_zones();

// Search within specific zones
let results = index.cross_zone_search(&query, k, &[zone_0, zone_1]);
```

### 4. Dynamic Updates

Efficiently handle graph changes with incremental cut recomputation:

```rust
// Single edge update
index.add_edge(u, v, weight);
index.remove_edge(u, v);

// Batch updates
let updates = vec![
    EdgeUpdate { kind: UpdateKind::Insert, u: 0, v: 1, weight: Some(0.8) },
    EdgeUpdate { kind: UpdateKind::Delete, u: 2, v: 3, weight: None },
];
let stats = index.batch_update(updates);
```

### 5. Cut Pruning

Remove weak edges to improve coherence:

```rust
let pruned_count = index.prune_weak_edges(threshold);
```

## Performance Characteristics

### Time Complexity

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| Insert | O(log n × M) | Same as HNSW |
| Search (ungated) | O(log n) | Same as HNSW |
| Search (gated) | O(log n) | Plus gate checks |
| Min-cut | O(n³) | Stoer-Wagner, cached |
| Zone computation | O(n²) | Periodic recomputation |

### Space Complexity

- **Base HNSW**: O(n × M × L) where L is layer count
- **Cut tracking**: O(n²) for adjacency (sparse in practice)
- **Total**: O(n × M × L + e) where e is edge count

### Optimizations

1. **Cached Min-Cut**: Recomputes only when graph changes
2. **Incremental Updates**: Version-tracked cache invalidation
3. **Sparse Adjacency**: HashMap-based for efficiency
4. **Periodic Recomputation**: Configurable via `cut_recompute_interval`

## Use Cases

### 1. Multi-Domain Discovery

Search within specific research domains without crossing into others:

```rust
// Climate papers in one cluster, finance in another
// Query climate without getting finance results
let climate_results = index.search_gated(&climate_query, 10);
```

### 2. Anomaly Detection

Identify nodes that bridge disparate clusters:

```rust
let zones = index.compute_zones();
for zone in zones {
    if zone.coherence_ratio < threshold {
        // Low coherence = potential boundary/anomaly
    }
}
```

### 3. Hierarchical Exploration

Navigate from abstract to specific within a coherent region:

```rust
let l1_neighbors = index.coherent_neighborhood(root, 1);
let l2_neighbors = index.coherent_neighborhood(root, 2);
// Expand without crossing semantic boundaries
```

### 4. Cross-Domain Linking

Explicitly find connections between domains:

```rust
// Find papers that bridge climate and finance
let bridging_papers = index.cross_zone_search(
    &interdisciplinary_query,
    10,
    &[climate_zone, finance_zone]
);
```

## Metrics and Monitoring

Track performance and behavior:

```rust
let metrics = index.metrics();
println!("Searches: {}", metrics.searches_performed.load(Ordering::Relaxed));
println!("Gates triggered: {}", metrics.cut_gates_triggered.load(Ordering::Relaxed));
println!("Expansions pruned: {}", metrics.expansions_pruned.load(Ordering::Relaxed));

// Export as JSON
let json = index.export_metrics();

// Get cut distribution
let dist = index.cut_distribution();
for layer_stats in dist {
    println!("Layer {}: avg_cut={:.3}", layer_stats.layer, layer_stats.avg_cut);
}
```

## Configuration Guide

### CutAwareConfig Parameters

```rust
pub struct CutAwareConfig {
    // Standard HNSW
    pub m: usize,                    // Max connections per node (default: 16)
    pub ef_construction: usize,      // Construction quality (default: 200)
    pub ef_search: usize,            // Search quality (default: 50)

    // Cut-aware
    pub coherence_gate_threshold: f64,    // Weak cut threshold (default: 0.3)
    pub max_cross_cut_hops: usize,        // Max boundary crossings (default: 2)
    pub enable_cut_pruning: bool,         // Auto-prune weak edges (default: false)
    pub cut_recompute_interval: usize,    // Recompute frequency (default: 100)
    pub min_zone_size: usize,             // Min nodes per zone (default: 5)
}
```

### Tuning Guidelines

| Workload | `coherence_gate_threshold` | `max_cross_cut_hops` | Notes |
|----------|---------------------------|---------------------|-------|
| Strict coherence | 0.5-0.8 | 0-1 | Stay within zones |
| Moderate | 0.3-0.5 | 2-3 | Some flexibility |
| Exploratory | 0.1-0.3 | 3-5 | Cross boundaries |
| No gating | 0.0 | ∞ | Ungated search |

## Examples

### Basic Usage

```rust
use ruvector_data_framework::cut_aware_hnsw::{CutAwareHNSW, CutAwareConfig};

let config = CutAwareConfig::default();
let mut index = CutAwareHNSW::new(config);

// Build index
for i in 0..100 {
    let vector = generate_vector(i);
    index.insert(i as u32, &vector)?;
}

// Query
let results = index.search_gated(&query, 10);
for result in results {
    println!("Node {}: distance={:.4}, coherence={:.3}",
        result.node_id, result.distance, result.coherence_score);
}
```

### Advanced: Multi-Cluster Discovery

See `examples/cut_aware_demo.rs` for a complete example demonstrating:
- Three distinct semantic clusters
- Gated vs ungated search comparison
- Coherent neighborhood exploration
- Cross-zone queries
- Metrics tracking

## Testing

The implementation includes 16 comprehensive tests:

```bash
cargo test --lib cut_aware_hnsw
```

**Test Coverage:**
- ✅ Dynamic cut watcher (basic, partition, triangle)
- ✅ Cut-aware insert and search
- ✅ Gated vs ungated comparison
- ✅ Coherent neighborhoods
- ✅ Zone computation
- ✅ Cross-zone search
- ✅ Edge updates (single and batch)
- ✅ Weak edge pruning
- ✅ Metrics tracking and export
- ✅ Boundary edge identification

## Benchmarks

Compare gated vs ungated search performance:

```bash
cargo bench --bench cut_aware_hnsw_bench
```

**Benchmarks:**
- Gated vs ungated search (100, 500, 1000 nodes)
- Coherent neighborhood (radius 2, 5)
- Zone computation
- Batch updates (10, 50, 100 edges)
- Cross-zone search

**Expected Results:**
- Ungated search: ~10-50 μs for 1000 nodes
- Gated search: ~15-70 μs (overhead from gate checks)
- Zone computation: ~1-5 ms for 1000 nodes

## Integration with RuVector

### With ruvector-core

```rust
// Use ruvector-core for production HNSW
use ruvector_core::hnsw::HnswIndex as RuvectorHNSW;

// Wrap with cut-awareness
let base_index = RuvectorHNSW::new(dimension);
let cut_aware = CutAwareHNSW::with_base(base_index, config);
```

### With ruvector-mincut

```rust
// Use ruvector-mincut for production min-cut
use ruvector_mincut::StoerWagner;

// Replace DynamicCutWatcher backend
let mincut = StoerWagner::new();
let watcher = DynamicCutWatcher::with_backend(mincut);
```

## Limitations

1. **Min-Cut Complexity**: O(n³) Stoer-Wagner limits scalability to ~10k nodes
2. **Memory**: Stores full adjacency (sparse) for cut computation
3. **Static Partitions**: Zones recomputed periodically, not incrementally
4. **Threshold Sensitivity**: Results depend on `coherence_gate_threshold`

## Future Enhancements

### Planned Features

1. **Euler Tour Trees** - O(log n) dynamic connectivity for faster updates
2. **Hierarchical Cuts** - Multi-level zone hierarchy
3. **Approximate Min-Cut** - Karger's algorithm for large graphs
4. **Persistent Zones** - Incremental zone maintenance
5. **SIMD Distance** - Accelerated vector comparisons

### Research Directions

1. **Learned Gates** - ML-based coherence threshold prediction
2. **Temporal Coherence** - Track coherence evolution over time
3. **Multi-Metric Cuts** - Combine similarity, citation, correlation
4. **Distributed Cuts** - Partition across machines

## References

1. **Stoer-Wagner Algorithm**
   - Stoer & Wagner (1997). "A simple min-cut algorithm"

2. **HNSW**
   - Malkov & Yashunin (2018). "Efficient and robust approximate nearest neighbor search"

3. **Dynamic Connectivity**
   - Holm et al. (2001). "Poly-logarithmic deterministic fully-dynamic algorithms"

4. **Applications**
   - Cross-domain research discovery
   - Hierarchical document clustering
   - Anomaly detection in graphs

## License

Same as RuVector (MIT/Apache-2.0)

## Contributing

See `CONTRIBUTING.md` for guidelines on:
- Adding new distance metrics
- Optimizing cut algorithms
- Improving zone computation
- Adding tests and benchmarks
