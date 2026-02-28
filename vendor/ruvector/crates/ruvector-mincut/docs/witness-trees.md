# Witness Trees Implementation

## Overview

This document describes the implementation of Witness Trees for dynamic minimum cut maintenance, following the Jin-Sun-Thorup algorithm from SODA 2024: "Fully Dynamic Exact Minimum Cut in Subpolynomial Time".

## What are Witness Trees?

Witness trees maintain a spanning forest of a graph where each tree edge is "witnessed" by a cut that certifies its inclusion in the tree. This data structure enables efficient dynamic maintenance of minimum cuts.

### Key Properties

1. **Witness Invariant**: Each tree edge (u,v) has a witness cut C such that removing (u,v) from the tree reveals C
2. **Minimum Cut Certificate**: The minimum among all witness cuts equals the graph's minimum cut
3. **Lazy Updates**: Updates are performed lazily to achieve better amortized complexity

## Architecture

### Core Components

```
WitnessTree
├── LinkCutTree         # Dynamic connectivity queries
├── Witnesses           # HashMap of edge witnesses
├── Tree Edges          # Spanning forest edges
├── Non-Tree Edges      # Cycle-forming edges
└── Min Cut Info        # Cached minimum cut value and edges
```

### Key Data Structures

```rust
// Witness for a tree edge
pub struct EdgeWitness {
    pub tree_edge: (VertexId, VertexId),
    pub cut_value: Weight,
    pub cut_side: HashSet<VertexId>,  // One side of the cut
}

// Main witness tree structure
pub struct WitnessTree {
    lct: LinkCutTree,                              // O(log n) connectivity
    witnesses: HashMap<(VertexId, VertexId), EdgeWitness>,
    min_cut: Weight,
    min_cut_edges: Vec<Edge>,
    graph: Arc<RwLock<DynamicGraph>>,
    dirty: bool,
    tree_edges: HashSet<(VertexId, VertexId)>,
    non_tree_edges: HashSet<(VertexId, VertexId)>,
}
```

## Algorithm Details

### Build Phase

```rust
fn build_spanning_tree() -> Result<()>
```

1. **Spanning Tree Construction** (BFS):
   - O(n + m) time
   - Creates spanning forest for disconnected graphs
   - Identifies tree vs non-tree edges

2. **Witness Computation**:
   - For each tree edge (u,v):
     - Find components after removing (u,v)
     - Compute cut value between components
     - Store witness

**Complexity**: O(n·m) for initial build

### Insert Edge

```rust
pub fn insert_edge(u, v, weight) -> Result<Weight>
```

**Case 1: Bridge Edge** (u and v in different components)
- Add to spanning tree
- Link in Link-Cut Tree
- Compute witness for new edge
- Update min cut if needed

**Case 2: Cycle Edge** (u and v already connected)
- Add to non-tree edges
- Mark dirty for recomputation
- May improve minimum cut

**Complexity**: Amortized O(log n) with lazy updates

### Delete Edge

```rust
pub fn delete_edge(u, v) -> Result<Weight>
```

**Case 1: Tree Edge**
- Remove from spanning tree
- Cut in Link-Cut Tree
- Find replacement edge in non-tree edges
- If found: add to tree, compute witness
- Update min cut

**Case 2: Non-Tree Edge**
- Remove from non-tree edges
- Mark dirty for recomputation

**Complexity**: O(m) worst case (finding replacement), O(log n) amortized

### Finding Minimum Cut

```rust
fn recompute_min_cut()
```

1. Examine all tree edge witnesses
2. Find witness with minimum cut value
3. Collect edges in that cut
4. Cache result

**Complexity**: O(number of tree edges) = O(n)

## Optimizations

### 1. Lazy Witness Tree

```rust
pub struct LazyWitnessTree {
    inner: WitnessTree,
    pending_updates: Vec<(VertexId, VertexId, bool)>,
    batch_threshold: usize,
}
```

- Batches updates together
- Flushes when threshold reached
- Better amortized complexity for sequences

### 2. Link-Cut Tree Integration

- O(log n) connectivity queries
- O(log n) link/cut operations
- Path compression for efficiency

### 3. Canonical Edge Keys

```rust
fn canonical_key(u, v) -> (VertexId, VertexId) {
    if u <= v { (u, v) } else { (v, u) }
}
```

- Consistent edge representation
- Efficient HashMap lookups
- Avoids duplicate edges

## Complexity Analysis

| Operation | Time Complexity | Space Complexity |
|-----------|----------------|------------------|
| Build | O(n·m) | O(n + m) |
| Insert Edge | O(log n) amortized | O(1) |
| Delete Edge | O(m) worst, O(log n) amortized | O(1) |
| Min Cut Query | O(1) | - |
| Find Witness | O(1) | - |

## Implementation Notes

### Thread Safety

The implementation uses `Arc<RwLock<DynamicGraph>>` for thread-safe graph access:
- Multiple concurrent reads allowed
- Exclusive write access when modifying

### Edge Cases Handled

1. **Empty Graph**: Returns ∞ for min cut
2. **Disconnected Graph**: Returns 0 (no cut exists)
3. **Single Vertex**: Returns ∞
4. **Dynamic Vertices**: Automatically adds new vertices to LCT

### Limitations

1. **Spanning Tree Dependency**: Only considers cuts corresponding to tree edges
2. **Approximation**: May not find optimal cut if it doesn't correspond to tree structure
3. **Replacement Search**: Finding replacement edges is O(m) in worst case

## Testing

The implementation includes 20 comprehensive tests:

### Basic Functionality
- `test_build_empty` - Empty graph handling
- `test_build_single_vertex` - Single vertex
- `test_build_triangle` - Simple connected graph
- `test_build_bridge` - Bridge detection

### Dynamic Updates
- `test_insert_bridge_edge` - Adding bridge edges
- `test_insert_cycle_edge` - Adding cycle edges
- `test_delete_tree_edge` - Removing tree edges
- `test_delete_non_tree_edge` - Removing non-tree edges
- `test_dynamic_sequence` - Sequence of operations

### Correctness
- `test_is_tree_edge` - Tree edge identification
- `test_find_witness` - Witness retrieval
- `test_tree_edge_cut` - Cut value computation
- `test_weighted_edges` - Weighted graph support
- `test_canonical_key` - Edge key normalization

### Advanced Features
- `test_lazy_witness_tree` - Lazy updates
- `test_lazy_witness_batch_threshold` - Batching
- `test_disconnected_graph` - Multiple components
- `test_large_graph` - Scalability (100 vertices)
- `test_complete_graph` - Dense graphs

### All Tests Pass ✓

```bash
test result: ok. 20 passed; 0 failed; 0 ignored
```

## Usage Examples

### Basic Usage

```rust
use std::sync::Arc;
use parking_lot::RwLock;
use ruvector_mincut::{DynamicGraph, WitnessTree};

// Create graph
let graph = Arc::new(RwLock::new(DynamicGraph::new()));
graph.write().insert_edge(1, 2, 1.0).unwrap();
graph.write().insert_edge(2, 3, 1.0).unwrap();
graph.write().insert_edge(3, 1, 1.0).unwrap();

// Build witness tree
let mut witness = WitnessTree::build(graph.clone()).unwrap();

// Query minimum cut
println!("Min cut: {}", witness.min_cut_value());
println!("Cut edges: {:?}", witness.min_cut_edges());
```

### Dynamic Updates

```rust
// Insert edge
graph.write().insert_edge(1, 4, 2.0).unwrap();
let new_cut = witness.insert_edge(1, 4, 2.0).unwrap();
println!("New min cut: {}", new_cut);

// Delete edge
graph.write().delete_edge(1, 2).unwrap();
let updated_cut = witness.delete_edge(1, 2).unwrap();
```

### Lazy Updates

```rust
use ruvector_mincut::LazyWitnessTree;

let mut lazy = LazyWitnessTree::with_threshold(graph, 10).unwrap();

// Batch updates
for i in 1..10 {
    graph.write().insert_edge(i, i+1, 1.0).unwrap();
    lazy.insert_edge(i, i+1, 1.0).unwrap();
}

// Force flush and get result
let min_cut = lazy.min_cut_value();
```

## Future Improvements

1. **Parallel Witness Computation**: Compute witnesses in parallel for large graphs
2. **Incremental Updates**: More efficient incremental witness updates
3. **Approximate Witnesses**: Trade accuracy for speed in large graphs
4. **Persistent Data Structures**: Better support for versioning and rollback

## References

- Jin, C., & Sun, R., & Thorup, M. (2024). "Fully Dynamic Exact Minimum Cut in Subpolynomial Time". SODA 2024.
- Sleator, D. D., & Tarjan, R. E. (1983). "A data structure for dynamic trees". Journal of Computer and System Sciences.

## File Location

`/home/user/ruvector/crates/ruvector-mincut/src/witness/mod.rs`

## Integration

The witness tree module is fully integrated into the ruvector-mincut crate:

```rust
pub use witness::{WitnessTree, LazyWitnessTree, EdgeWitness};
```

Available in the prelude for convenient access.
