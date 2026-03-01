# API Quick Reference

Complete reference for all public types in ruvector-mincut.

---

## 1. Core Types

### MinCutBuilder

Builder pattern for creating `DynamicMinCut` instances.

| Method | Description | Example |
|--------|-------------|---------|
| `new()` | Create new builder | `MinCutBuilder::new()` |
| `exact()` | Use exact algorithm | `.exact()` |
| `approximate(ε)` | Use (1+ε)-approximate algorithm | `.approximate(0.1)` |
| `max_cut_size(n)` | Set max cut size for exact mode | `.max_cut_size(1000)` |
| `parallel(bool)` | Enable/disable parallel computation | `.parallel(true)` |
| `with_edges(vec)` | Initialize with edges | `.with_edges(vec![(1,2,1.0)])` |
| `build()` | Build the structure | `.build()` |

**Example:**
```rust
use ruvector_mincut::MinCutBuilder;

let mincut = MinCutBuilder::new()
    .exact()
    .max_cut_size(500)
    .parallel(true)
    .with_edges(vec![(1, 2, 1.0), (2, 3, 1.0)])
    .build()?;
```

---

### DynamicMinCut

Main dynamic minimum cut structure.

| Method | Return Type | Description | Example |
|--------|-------------|-------------|---------|
| `new(config)` | `Self` | Create with config | `DynamicMinCut::new(config)` |
| `from_graph(g, cfg)` | `Result<Self>` | Build from existing graph | `DynamicMinCut::from_graph(graph, config)` |
| `insert_edge(u, v, w)` | `Result<f64>` | Insert edge, returns new min cut | `.insert_edge(1, 2, 1.0)?` |
| `delete_edge(u, v)` | `Result<f64>` | Delete edge, returns new min cut | `.delete_edge(1, 2)?` |
| `min_cut_value()` | `f64` | Get current min cut (O(1)) | `.min_cut_value()` |
| `min_cut()` | `MinCutResult` | Get detailed result | `.min_cut()` |
| `partition()` | `(Vec<u64>, Vec<u64>)` | Get cut partition | `.partition()` |
| `cut_edges()` | `Vec<Edge>` | Get edges in the cut | `.cut_edges()` |
| `is_connected()` | `bool` | Check connectivity | `.is_connected()` |
| `num_vertices()` | `usize` | Vertex count | `.num_vertices()` |
| `num_edges()` | `usize` | Edge count | `.num_edges()` |
| `stats()` | `AlgorithmStats` | Performance statistics | `.stats()` |
| `reset_stats()` | `()` | Reset statistics | `.reset_stats()` |
| `config()` | `&MinCutConfig` | Get configuration | `.config()` |
| `graph()` | `Arc<RwLock<Graph>>` | Get graph reference | `.graph()` |

**Example:**
```rust
let mut mincut = MinCutBuilder::new()
    .with_edges(vec![(1, 2, 1.0)])
    .build()?;

let new_cut = mincut.insert_edge(2, 3, 1.0)?;
println!("New min cut: {}", new_cut);

let result = mincut.min_cut();
let (s, t) = mincut.partition();
```

---

### MinCutResult

Result of a minimum cut query.

| Field | Type | Description |
|-------|------|-------------|
| `value` | `f64` | Minimum cut value |
| `cut_edges` | `Option<Vec<Edge>>` | Edges in the cut |
| `partition` | `Option<(Vec<u64>, Vec<u64>)>` | Vertex partition (S, T) |
| `is_exact` | `bool` | Whether result is exact |
| `approximation_ratio` | `f64` | Approximation ratio (1.0 if exact) |

**Example:**
```rust
let result = mincut.min_cut();
println!("Min cut value: {}", result.value);
println!("Is exact: {}", result.is_exact);
if let Some((s, t)) = result.partition {
    println!("Partition sizes: {} and {}", s.len(), t.len());
}
```

---

### MinCutConfig

Configuration for minimum cut algorithm.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_exact_cut_size` | `usize` | `1000` | Max cut size for exact algorithm |
| `epsilon` | `f64` | `0.1` | Approximation parameter (0 < ε ≤ 1) |
| `approximate` | `bool` | `false` | Use approximate mode |
| `parallel` | `bool` | `true` | Enable parallel computation |
| `cache_size` | `usize` | `10000` | Cache size for intermediate results |

**Example:**
```rust
use ruvector_mincut::MinCutConfig;

let config = MinCutConfig {
    max_exact_cut_size: 500,
    epsilon: 0.2,
    approximate: false,
    parallel: true,
    cache_size: 5000,
};
```

---

### AlgorithmStats

Performance statistics.

| Field | Type | Description |
|-------|------|-------------|
| `insertions` | `u64` | Total insertions performed |
| `deletions` | `u64` | Total deletions performed |
| `queries` | `u64` | Total queries performed |
| `avg_update_time_us` | `f64` | Average update time (microseconds) |
| `avg_query_time_us` | `f64` | Average query time (microseconds) |
| `restructures` | `u64` | Number of tree restructures |

**Example:**
```rust
let stats = mincut.stats();
println!("Updates: {} insertions, {} deletions",
    stats.insertions, stats.deletions);
println!("Avg update time: {:.2}μs", stats.avg_update_time_us);
```

---

## 2. Graph Types

### DynamicGraph

Thread-safe dynamic graph structure.

| Method | Return Type | Description | Example |
|--------|-------------|-------------|---------|
| `new()` | `Self` | Create empty graph | `DynamicGraph::new()` |
| `with_capacity(v, e)` | `Self` | Create with capacity hint | `.with_capacity(100, 200)` |
| `add_vertex(v)` | `bool` | Add vertex (returns true if new) | `.add_vertex(1)` |
| `has_vertex(v)` | `bool` | Check if vertex exists | `.has_vertex(1)` |
| `insert_edge(u, v, w)` | `Result<EdgeId>` | Insert weighted edge | `.insert_edge(1, 2, 1.0)?` |
| `delete_edge(u, v)` | `Result<Edge>` | Delete edge | `.delete_edge(1, 2)?` |
| `has_edge(u, v)` | `bool` | Check if edge exists | `.has_edge(1, 2)` |
| `get_edge(u, v)` | `Option<Edge>` | Get edge by endpoints | `.get_edge(1, 2)` |
| `neighbors(v)` | `Vec<(u64, EdgeId)>` | Get vertex neighbors | `.neighbors(1)` |
| `degree(v)` | `usize` | Get vertex degree | `.degree(1)` |
| `num_vertices()` | `usize` | Vertex count | `.num_vertices()` |
| `num_edges()` | `usize` | Edge count | `.num_edges()` |
| `vertices()` | `Vec<VertexId>` | All vertices | `.vertices()` |
| `edges()` | `Vec<Edge>` | All edges | `.edges()` |
| `stats()` | `GraphStats` | Graph statistics | `.stats()` |
| `is_connected()` | `bool` | Check connectivity | `.is_connected()` |
| `connected_components()` | `Vec<Vec<VertexId>>` | Find components | `.connected_components()` |
| `clear()` | `()` | Clear all data | `.clear()` |
| `remove_vertex(v)` | `Result<()>` | Remove vertex and incident edges | `.remove_vertex(1)?` |
| `edge_weight(u, v)` | `Option<f64>` | Get edge weight | `.edge_weight(1, 2)` |
| `update_edge_weight(u, v, w)` | `Result<()>` | Update edge weight | `.update_edge_weight(1, 2, 2.0)?` |

**Example:**
```rust
let graph = DynamicGraph::new();
graph.add_vertex(1);
graph.add_vertex(2);
let edge_id = graph.insert_edge(1, 2, 1.5)?;
println!("Created edge {}", edge_id);
```

---

### Edge

An edge in the graph.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `EdgeId` (`u64`) | Unique edge identifier |
| `source` | `VertexId` (`u64`) | Source vertex |
| `target` | `VertexId` (`u64`) | Target vertex |
| `weight` | `Weight` (`f64`) | Edge weight |

| Method | Return Type | Description |
|--------|-------------|-------------|
| `new(id, src, tgt, w)` | `Self` | Create new edge |
| `canonical_endpoints()` | `(u64, u64)` | Get ordered endpoints |
| `other(v)` | `Option<u64>` | Get other endpoint |

**Example:**
```rust
use ruvector_mincut::Edge;

let edge = Edge::new(0, 1, 2, 1.5);
assert_eq!(edge.canonical_endpoints(), (1, 2));
assert_eq!(edge.other(1), Some(2));
```

---

### GraphStats

Graph statistics.

| Field | Type | Description |
|-------|------|-------------|
| `num_vertices` | `usize` | Number of vertices |
| `num_edges` | `usize` | Number of edges |
| `total_weight` | `f64` | Sum of all edge weights |
| `min_degree` | `usize` | Minimum vertex degree |
| `max_degree` | `usize` | Maximum vertex degree |
| `avg_degree` | `f64` | Average vertex degree |

**Example:**
```rust
let stats = graph.stats();
println!("Graph: {} vertices, {} edges",
    stats.num_vertices, stats.num_edges);
println!("Degree range: {} to {}",
    stats.min_degree, stats.max_degree);
```

---

### Type Aliases

| Type | Alias | Description |
|------|-------|-------------|
| `VertexId` | `u64` | Unique vertex identifier |
| `EdgeId` | `u64` | Unique edge identifier |
| `Weight` | `f64` | Edge weight type |

---

## 3. Algorithm Variants

### ApproxMinCut

(1+ε)-approximate minimum cut for all cut sizes (SODA 2025).

| Method | Return Type | Description | Example |
|--------|-------------|-------------|---------|
| `new(config)` | `Self` | Create with config | `ApproxMinCut::new(config)` |
| `with_epsilon(ε)` | `Self` | Create with specific ε | `ApproxMinCut::with_epsilon(0.1)` |
| `insert_edge(u, v, w)` | `()` | Insert edge | `.insert_edge(1, 2, 1.0)` |
| `delete_edge(u, v)` | `()` | Delete edge | `.delete_edge(1, 2)` |
| `min_cut()` | `ApproxMinCutResult` | Query min cut | `.min_cut()` |
| `min_cut_value()` | `f64` | Get value only | `.min_cut_value()` |
| `is_connected()` | `bool` | Check connectivity | `.is_connected()` |
| `vertex_count()` | `usize` | Vertex count | `.vertex_count()` |
| `edge_count()` | `usize` | Edge count | `.edge_count()` |
| `stats()` | `&ApproxMinCutStats` | Get statistics | `.stats()` |

**Example:**
```rust
use ruvector_mincut::ApproxMinCut;

let mut approx = ApproxMinCut::with_epsilon(0.1);
approx.insert_edge(1, 2, 1.0);
approx.insert_edge(2, 3, 1.0);

let result = approx.min_cut();
println!("Approx cut: {}", result.value);
println!("Bounds: [{}, {}]", result.lower_bound, result.upper_bound);
```

---

### ApproxMinCutConfig

Configuration for approximate algorithm.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `epsilon` | `f64` | `0.1` | Approximation parameter |
| `num_samples` | `usize` | `3` | Number of sparsifier samples |
| `seed` | `u64` | `42` | Random seed |

---

### ApproxMinCutResult

Result from approximate algorithm.

| Field | Type | Description |
|-------|------|-------------|
| `value` | `f64` | Approximate cut value |
| `lower_bound` | `f64` | Lower bound (value / (1+ε)) |
| `upper_bound` | `f64` | Upper bound (value × (1+ε)) |
| `partition` | `Option<(Vec<u64>, Vec<u64>)>` | Partition if computed |
| `epsilon` | `f64` | Approximation ratio used |

---

### PolylogConnectivity

Polylogarithmic worst-case dynamic connectivity.

| Method | Return Type | Description | Time Complexity |
|--------|-------------|-------------|-----------------|
| `new()` | `Self` | Create new structure | O(1) |
| `insert(u, v)` | `Result<()>` | Insert edge | O(log³ n) expected |
| `delete(u, v)` | `Result<()>` | Delete edge | O(log³ n) expected |
| `connected(u, v)` | `bool` | Query connectivity | O(log n) |
| `stats()` | `PolylogStats` | Get statistics | O(1) |

**Example:**
```rust
use ruvector_mincut::PolylogConnectivity;

let mut conn = PolylogConnectivity::new();
conn.insert(1, 2)?;
conn.insert(2, 3)?;
assert!(conn.connected(1, 3));
```

---

### PolylogStats

Statistics for polylog connectivity.

| Field | Type | Description |
|-------|------|-------------|
| `num_levels` | `usize` | Number of hierarchy levels |
| `total_edges` | `usize` | Total edges across all levels |
| `rebuilds` | `u64` | Number of level rebuilds |
| `avg_insert_time_us` | `f64` | Average insertion time |
| `avg_delete_time_us` | `f64` | Average deletion time |

---

## 4. Paper Implementation Types

### ThreeLevelHierarchy

3-level cluster decomposition (arXiv:2512.13105).

| Type | Description |
|------|-------------|
| `Expander` | Level 0: φ-expander subgraphs |
| `Precluster` | Level 1: Groups of expanders |
| `HierarchyCluster` | Level 2: Top-level clusters |

#### Expander

| Field | Type | Description |
|-------|------|-------------|
| `id` | `u64` | Unique ID |
| `vertices` | `HashSet<VertexId>` | Vertices in expander |
| `internal_edges` | `Vec<(u64, u64)>` | Internal edges |
| `boundary_edges` | `Vec<(u64, u64)>` | Boundary edges |
| `volume` | `usize` | Sum of degrees |
| `expansion_ratio` | `f64` | Verified expansion φ |
| `precluster_id` | `Option<u64>` | Parent precluster |

| Method | Description |
|--------|-------------|
| `new(id, vertices)` | Create new expander |
| `size()` | Number of vertices |
| `contains(v)` | Check membership |
| `boundary_sparsity()` | Compute boundary/volume ratio |

---

#### Precluster

| Field | Type | Description |
|-------|------|-------------|
| `id` | `u64` | Unique ID |
| `expanders` | `Vec<u64>` | Expander IDs |
| `vertices` | `HashSet<VertexId>` | All vertices |
| `boundary_edges` | `Vec<(u64, u64)>` | Boundary edges |
| `volume` | `usize` | Total volume |
| `cluster_id` | `Option<u64>` | Parent cluster |

| Method | Description |
|--------|-------------|
| `new(id)` | Create new precluster |
| `add_expander(exp)` | Add expander |
| `size()` | Number of vertices |
| `boundary_ratio()` | Boundary/volume ratio |

---

#### HierarchyCluster

| Field | Type | Description |
|-------|------|-------------|
| `id` | `u64` | Unique ID |
| `preclusters` | `Vec<u64>` | Precluster IDs |
| `vertices` | `HashSet<VertexId>` | All vertices |
| `boundary_edges` | `Vec<(u64, u64)>` | Boundary edges |
| `mirror_cuts` | `Vec<MirrorCut>` | Tracked cuts |
| `internal_min_cut` | `f64` | Internal min cut |

---

### DeterministicLocalKCut

Deterministic local minimum cut algorithm.

| Type | Description |
|------|-------------|
| `EdgeColor` | Edge coloring (Red, Blue, Green, Yellow) |
| `EdgeColoring` | Coloring assignment with parameters a, b |
| `GreedyForestPacking` | Forest packing structure |

#### EdgeColor

```rust
pub enum EdgeColor {
    Red,    // Forest edge, class 1
    Blue,   // Forest edge, class 2
    Green,  // Non-forest edge, class 1
    Yellow, // Non-forest edge, class 2
}
```

#### EdgeColoring

| Field | Type | Description |
|-------|------|-------------|
| `a` | `usize` | Cut size parameter |
| `b` | `usize` | Volume parameter |

| Method | Description |
|--------|-------------|
| `new(a, b)` | Create new coloring |
| `get(u, v)` | Get edge color |
| `set(u, v, color)` | Set edge color |
| `has_color(u, v, c)` | Check specific color |

---

#### GreedyForestPacking

| Field | Type | Description |
|-------|------|-------------|
| `num_forests` | `usize` | Number of forests (k) |

| Method | Description |
|--------|-------------|
| `new(k)` | Create k forests |
| `insert_edge(u, v)` | Insert into first available forest |
| `delete_edge(u, v)` | Remove edge |
| `is_tree_edge(u, v)` | Check if edge is in some forest |

**Example:**
```rust
use ruvector_mincut::GreedyForestPacking;

let mut packing = GreedyForestPacking::new(10);
if let Some(forest_id) = packing.insert_edge(1, 2) {
    println!("Edge added to forest {}", forest_id);
}
```

---

### MinCutWrapper

Wrapper managing O(log n) bounded instances.

| Method | Return Type | Description | Example |
|--------|-------------|-------------|---------|
| `new(graph)` | `Self` | Create with default factory | `MinCutWrapper::new(graph)` |
| `with_factory(g, f)` | `Self` | Create with custom factory | `.with_factory(g, factory)` |
| `insert_edge(id, u, v)` | `()` | Buffer edge insertion | `.insert_edge(0, 1, 2)` |
| `delete_edge(id, u, v)` | `()` | Buffer edge deletion | `.delete_edge(0, 1, 2)` |
| `query()` | `MinCutResult` | Query min cut | `.query()` |

**Example:**
```rust
use ruvector_mincut::{MinCutWrapper, DynamicGraph};
use std::sync::Arc;

let graph = Arc::new(DynamicGraph::new());
let edge_id = graph.insert_edge(1, 2, 1.0)?;

let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
wrapper.insert_edge(edge_id, 1, 2);

let result = wrapper.query();
println!("Min cut: {}", result.value());
```

---

### CutCertificate

Verifiable certificate for minimum cut.

| Field | Type | Description |
|-------|------|-------------|
| `witnesses` | `Vec<WitnessHandle>` | Candidate cuts maintained |
| `witness_summaries` | `Vec<WitnessSummary>` | Serializable summaries |
| `localkcut_responses` | `Vec<LocalKCutResponse>` | Proof of no smaller cut |
| `best_witness_idx` | `Option<usize>` | Index of best witness |
| `timestamp` | `SystemTime` | Creation timestamp |
| `version` | `u32` | Certificate version |

| Method | Description |
|--------|-------------|
| `new()` | Create empty certificate |
| `with_witnesses(w)` | Create with witnesses |
| `add_witness(w)` | Add witness |
| `add_localkcut_response(r)` | Add LocalKCut proof |
| `find_best_witness()` | Find smallest boundary |
| `verify()` | Verify certificate validity |

**Example:**
```rust
use ruvector_mincut::CutCertificate;

let mut cert = CutCertificate::new();
cert.add_witness(witness);
cert.find_best_witness();
println!("Certificate valid: {}", cert.verify());
```

---

## 5. Integration Types

### RuVectorGraphAnalyzer

Graph analysis for ruvector ecosystem.

| Method | Return Type | Description | Example |
|--------|-------------|-------------|---------|
| `new(graph)` | `Self` | Create analyzer | `RuVectorGraphAnalyzer::new(g)` |
| `from_similarity_matrix(...)` | `Self` | Build from similarity | `.from_similarity_matrix(&sim, n, 0.8)` |
| `from_knn(neighbors)` | `Self` | Build from k-NN graph | `.from_knn(&neighbors)` |
| `min_cut()` | `u64` | Compute min cut | `.min_cut()` |
| `partition()` | `Option<(Vec, Vec)>` | Get partition | `.partition()` |
| `is_well_connected(t)` | `bool` | Check if cut ≥ threshold | `.is_well_connected(5)` |
| `find_bridges()` | `Vec<EdgeId>` | Find bridge edges | `.find_bridges()` |
| `add_edge(u, v, w)` | `Result<EdgeId>` | Add edge | `.add_edge(1, 2, 1.0)?` |
| `remove_edge(u, v)` | `Result<()>` | Remove edge | `.remove_edge(1, 2)?` |
| `invalidate_cache()` | `()` | Clear cached results | `.invalidate_cache()` |

**Example:**
```rust
use ruvector_mincut::{RuVectorGraphAnalyzer, DynamicGraph};
use std::sync::Arc;

let graph = Arc::new(DynamicGraph::new());
let mut analyzer = RuVectorGraphAnalyzer::new(graph);

analyzer.add_edge(1, 2, 1.0)?;
println!("Min cut: {}", analyzer.min_cut());
```

---

### CommunityDetector

Detect communities using recursive min-cut.

| Method | Return Type | Description | Example |
|--------|-------------|-------------|---------|
| `new(graph)` | `Self` | Create detector | `CommunityDetector::new(g)` |
| `detect(min_size)` | `&[Vec<VertexId>]` | Detect communities | `.detect(5)` |
| `communities()` | `&[Vec<VertexId>]` | Get communities | `.communities()` |

**Example:**
```rust
use ruvector_mincut::{CommunityDetector, DynamicGraph};
use std::sync::Arc;

let graph = Arc::new(DynamicGraph::new());
let mut detector = CommunityDetector::new(graph);

let communities = detector.detect(5);
println!("Found {} communities", communities.len());
```

---

### GraphPartitioner

Partition graph for distributed processing.

| Method | Return Type | Description | Example |
|--------|-------------|-------------|---------|
| `new(graph)` | `Self` | Create partitioner | `GraphPartitioner::new(g)` |
| `partition(k)` | `Vec<Vec<VertexId>>` | Partition into k parts | `.partition(4)` |
| `balanced_partition(k)` | `Vec<Vec<VertexId>>` | Balanced k-way partition | `.balanced_partition(4)` |

**Example:**
```rust
use ruvector_mincut::{GraphPartitioner, DynamicGraph};
use std::sync::Arc;

let graph = Arc::new(DynamicGraph::new());
let mut partitioner = GraphPartitioner::new(graph);

let parts = partitioner.balanced_partition(4);
println!("Partition sizes: {:?}",
    parts.iter().map(|p| p.len()).collect::<Vec<_>>());
```

---

## 6. Compact/Parallel Types (agentic feature)

### BitSet256

Compact bit-packed set for 256 vertices (32 bytes).

| Method | Description | Example |
|--------|-------------|---------|
| `new()` | Create empty set | `BitSet256::new()` |
| `insert(v)` | Insert vertex | `.insert(42)` |
| `contains(v)` | Check membership | `.contains(42)` |
| `remove(v)` | Remove vertex | `.remove(42)` |
| `count()` | Count members | `.count()` |
| `union(other)` | Set union | `.union(&other)` |
| `intersection(other)` | Set intersection | `.intersection(&other)` |
| `xor(other)` | Symmetric difference | `.xor(&other)` |
| `iter()` | Iterate members | `.iter()` |

**Example:**
```rust
use ruvector_mincut::BitSet256;

let mut set = BitSet256::new();
set.insert(10);
set.insert(20);
assert_eq!(set.count(), 2);
assert!(set.contains(10));
```

---

### CompactCoreState

Complete min-cut state for 8KB WASM core.

| Field | Type | Description |
|-------|------|-------------|
| `vertices` | `BitSet256` | Active vertices |
| `edges` | `[CompactEdge; MAX_EDGES]` | Edge array |
| `num_edges` | `u16` | Number of active edges |
| `witnesses` | `[CompactWitness; 8]` | Maintained witnesses |
| `num_witnesses` | `u8` | Number of witnesses |
| `current_min_cut` | `u16` | Current min cut value |

| Constant | Value | Description |
|----------|-------|-------------|
| `MAX_VERTICES_PER_CORE` | `256` | Max vertices per core |
| `MAX_EDGES_PER_CORE` | `384` | Max edges per core |

---

### CompactEdge

Compact edge (8 bytes).

| Field | Type | Description |
|-------|------|-------------|
| `source` | `u16` | Source vertex |
| `target` | `u16` | Target vertex |
| `weight` | `u16` | Weight (0.01 precision) |
| `flags` | `u16` | Status flags |

| Flag | Value | Description |
|------|-------|-------------|
| `FLAG_ACTIVE` | `0x0001` | Edge is active |
| `FLAG_IN_CUT` | `0x0002` | Edge is in current cut |
| `FLAG_TREE_EDGE` | `0x0004` | Edge is in spanning tree |

---

### CoreDistributor

Distributes work across 256 cores.

| Field | Type | Description |
|-------|------|-------------|
| `strategy` | `CoreStrategy` | Distribution strategy |
| `num_vertices` | `u16` | Total vertices |
| `num_edges` | `u16` | Total edges |

| Method | Description |
|--------|-------------|
| `new(strategy, v, e)` | Create distributor |
| `vertex_to_core(v)` | Determine core for vertex |
| `distribute_ranges()` | Distribute geometric ranges |

---

### CoreExecutor

Execute min-cut on single core.

| Method | Return Type | Description |
|--------|-------------|-------------|
| `new(id, range)` | `Self` | Create executor for core |
| `load_state(state)` | `()` | Load graph state |
| `execute()` | `CoreResult` | Execute min-cut |
| `get_result()` | `CoreResult` | Get result |

---

### SharedCoordinator

Atomic coordination across cores (64 bytes).

| Field | Type | Description |
|-------|------|-------------|
| `global_min_cut` | `AtomicU16` | Global minimum found |
| `completed_cores` | `AtomicU8` | Completion count |
| `phase` | `AtomicU8` | Current phase |
| `best_core` | `AtomicU8` | Core with best result |

| Method | Description |
|--------|-------------|
| `new()` | Create coordinator |
| `try_update_min(val, id)` | Atomic min update |
| `mark_completed()` | Mark core done |
| `all_completed()` | Check if all done |

**Constants:**
```rust
pub const NUM_CORES: usize = 256;
pub const RANGES_PER_CORE: usize = 1;
pub const RANGE_FACTOR: f32 = 1.2;
```

---

### Helper Functions

| Function | Description |
|----------|-------------|
| `compute_core_range(id)` | Get (λ_min, λ_max) for core |

**Example:**
```rust
use ruvector_mincut::compute_core_range;

let (min, max) = compute_core_range(10);
println!("Core 10 handles range [{}, {}]", min, max);
```

---

## 7. Monitoring Types (monitoring feature)

### MonitorBuilder

Builder for `MinCutMonitor`.

| Method | Description | Example |
|--------|-------------|---------|
| `new()` | Create builder | `MonitorBuilder::new()` |
| `with_config(cfg)` | Set configuration | `.with_config(config)` |
| `threshold_below(v, name)` | Add below-threshold alert | `.threshold_below(10.0, "low")` |
| `threshold_above(v, name)` | Add above-threshold alert | `.threshold_above(100.0, "high")` |
| `on_change(name, cb)` | Add change callback | `.on_change("test", \|e\| {...})` |
| `on_event_type(type, name, cb)` | Add type-filtered callback | `.on_event_type(EventType::CutIncreased, "inc", \|e\| {...})` |
| `build()` | Build monitor | `.build()` |

**Example:**
```rust
use ruvector_mincut::{MonitorBuilder, EventType};

let monitor = MonitorBuilder::new()
    .threshold_below(5.0, "critical")
    .threshold_above(50.0, "high")
    .on_event_type(EventType::CutDecreased, "alert", |event| {
        println!("Cut decreased to {}", event.new_value);
    })
    .build();
```

---

### MinCutMonitor

Real-time monitoring for min-cut changes.

| Method | Return Type | Description | Example |
|--------|-------------|-------------|---------|
| `new(config)` | `Self` | Create monitor | `MinCutMonitor::new(cfg)` |
| `on_event(name, cb)` | `Result<()>` | Register callback | `.on_event("test", \|e\| {...})?` |
| `on_event_type(type, name, cb)` | `Result<()>` | Register filtered callback | `.on_event_type(EventType::CutIncreased, "inc", \|e\| {...})?` |
| `add_threshold(t)` | `Result<()>` | Add threshold | `.add_threshold(threshold)?` |
| `remove_threshold(name)` | `bool` | Remove threshold | `.remove_threshold("low")` |
| `remove_callback(name)` | `bool` | Remove callback | `.remove_callback("test")` |
| `notify(old, new, edge)` | `()` | Notify of change | `.notify(5.0, 10.0, None)` |
| `metrics()` | `MonitorMetrics` | Get metrics | `.metrics()` |
| `reset_metrics()` | `()` | Reset metrics | `.reset_metrics()` |
| `current_cut()` | `f64` | Get current value | `.current_cut()` |
| `threshold_status()` | `Vec<(String, bool)>` | Threshold states | `.threshold_status()` |

**Example:**
```rust
use ruvector_mincut::{MinCutMonitor, MonitorConfig};

let monitor = MinCutMonitor::new(MonitorConfig::default());

monitor.on_event("logger", |event| {
    println!("Event: {:?}", event.event_type);
})?;

monitor.notify(10.0, 5.0, None);
```

---

### EventType

Type of monitoring event.

```rust
pub enum EventType {
    CutIncreased,           // Min cut value increased
    CutDecreased,           // Min cut value decreased
    ThresholdCrossedBelow,  // Crossed below threshold
    ThresholdCrossedAbove,  // Crossed above threshold
    Disconnected,           // Graph became disconnected
    Connected,              // Graph became connected
    EdgeInserted,           // Edge was inserted
    EdgeDeleted,            // Edge was deleted
}
```

---

### MinCutEvent

Event from monitoring system.

| Field | Type | Description |
|-------|------|-------------|
| `event_type` | `EventType` | Type of event |
| `new_value` | `f64` | New min cut value |
| `old_value` | `f64` | Previous min cut value |
| `timestamp` | `Instant` | When event occurred |
| `threshold` | `Option<f64>` | Threshold crossed (if applicable) |
| `edge` | `Option<(u64, u64)>` | Edge involved (if applicable) |

---

### Threshold

Threshold configuration.

| Field | Type | Description |
|-------|------|-------------|
| `value` | `f64` | Threshold value |
| `name` | `String` | Identifier |
| `alert_below` | `bool` | Alert when below (vs above) |
| `enabled` | `bool` | Whether active |

| Method | Description |
|--------|-------------|
| `new(v, name, below)` | Create threshold |

**Example:**
```rust
use ruvector_mincut::Threshold;

let threshold = Threshold::new(10.0, "critical".to_string(), true);
```

---

### MonitorMetrics

Collected monitoring metrics.

| Field | Type | Description |
|-------|------|-------------|
| `total_events` | `u64` | Total events processed |
| `events_by_type` | `HashMap<String, u64>` | Events by type |
| `cut_history` | `Vec<(Instant, f64)>` | Sampled history |
| `avg_cut` | `f64` | Average cut value |
| `min_observed` | `f64` | Minimum observed |
| `max_observed` | `f64` | Maximum observed |
| `threshold_violations` | `u64` | Threshold crossings |
| `time_since_last_event` | `Option<Duration>` | Time since last event |

**Example:**
```rust
let metrics = monitor.metrics();
println!("Total events: {}", metrics.total_events);
println!("Average cut: {:.2}", metrics.avg_cut);
println!("Range: [{}, {}]", metrics.min_observed, metrics.max_observed);
```

---

### MonitorConfig

Monitor configuration.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_callbacks` | `usize` | `100` | Max registered callbacks |
| `sample_interval` | `Duration` | `1s` | History sampling interval |
| `max_history_size` | `usize` | `1000` | Max history entries |
| `collect_metrics` | `bool` | `true` | Enable metrics collection |

---

## Common Operation Quick Reference

### Initialize and Build

```rust
// Basic exact algorithm
let mincut = MinCutBuilder::new()
    .exact()
    .with_edges(vec![(1, 2, 1.0)])
    .build()?;

// Approximate algorithm
let approx = MinCutBuilder::new()
    .approximate(0.1)
    .with_edges(edges)
    .build()?;
```

### Dynamic Updates

```rust
// Insert edge
let new_cut = mincut.insert_edge(3, 4, 2.0)?;

// Delete edge
let new_cut = mincut.delete_edge(1, 2)?;

// Query
let value = mincut.min_cut_value();
let result = mincut.min_cut();
```

### Monitoring

```rust
let monitor = MonitorBuilder::new()
    .threshold_below(5.0, "critical")
    .on_change("log", |e| println!("{:?}", e))
    .build();

monitor.notify(old_value, new_value, None);
```

### Integration

```rust
let analyzer = RuVectorGraphAnalyzer::new(graph);
let cut = analyzer.min_cut();
let (s, t) = analyzer.partition().unwrap();

let detector = CommunityDetector::new(graph);
let communities = detector.detect(5);
```

---

## Error Handling

All fallible operations return `Result<T, MinCutError>`:

```rust
use ruvector_mincut::{MinCutBuilder, MinCutError};

match mincut.insert_edge(1, 2, 1.0) {
    Ok(new_cut) => println!("New cut: {}", new_cut),
    Err(MinCutError::EdgeExists(u, v)) => {
        println!("Edge {}-{} already exists", u, v);
    }
    Err(e) => println!("Error: {:?}", e),
}
```

Common errors:
- `EdgeExists(u, v)` - Edge already present
- `EdgeNotFound(u, v)` - Edge doesn't exist
- `InvalidEdge(u, v)` - Self-loop or invalid
- `InvalidVertex(v)` - Vertex doesn't exist
- `InvalidParameter(msg)` - Invalid configuration

---

## Performance Tips

1. **Use exact mode for small cuts** (<1000), approximate for larger
2. **Enable parallel** computation for graphs with >1000 edges
3. **Batch updates** when possible, query after batch complete
4. **Monitor selectively** - use event type filters
5. **For agentic chip**: Keep per-core state under 8KB
6. **Cache invalidation**: Only when necessary for integration types

---

## Version Information

```rust
use ruvector_mincut::{VERSION, NAME};

println!("{} v{}", NAME, VERSION);
```

Current certificate version: `CERTIFICATE_VERSION = 1`

---

## See Also

- [Getting Started Guide](01-getting-started.md)
- [Algorithm Details](02-algorithm-details.md)
- [Performance Guide](03-performance.md)
- [Paper Algorithms](04-paper-algorithms.md)
- [Agentic Chip](05-agentic-chip.md)
- [Monitoring Guide](06-monitoring.md)
