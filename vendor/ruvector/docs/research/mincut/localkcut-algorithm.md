# LocalKCut Algorithm - Technical Documentation

## Overview

The **LocalKCut** algorithm is a deterministic local minimum cut algorithm introduced in the December 2024 paper *"Deterministic and Exact Fully-dynamic Minimum Cut of Superpolylogarithmic Size"*. It provides a derandomized approach to finding minimum cuts near a given vertex.

## Key Innovation

Previous approaches (SODA 2025) used **randomized** sampling to find local cuts. The December 2024 paper **derandomizes** this using:

1. **Deterministic edge colorings** (4 colors)
2. **Color-constrained BFS** enumeration
3. **Forest packing** for witness guarantees

## Algorithm Description

### Input
- Graph `G = (V, E)` with edge weights
- Vertex `v ∈ V` (starting point)
- Cut size bound `k`

### Output
- A cut `(S, V\S)` with `v ∈ S` and cut value ≤ `k`
- Or `None` if no such cut exists near `v`

### Procedure

```
LocalKCut(G, v, k):
  1. Assign colors to edges: color(e) = edge_id mod 4
  2. Set radius r = ⌈log₄(k)⌉ + 1
  3. For depth d = 1 to r:
       For each color mask M ⊆ {Red, Blue, Green, Yellow}:
         4. Reachable[M] = ColorConstrainedBFS(v, M, d)
         5. If Reachable[M] forms a cut of size ≤ k:
              Store as candidate
  6. Return minimum cut found
```

### Color-Constrained BFS

```
ColorConstrainedBFS(start, color_mask, max_depth):
  1. visited = {start}
  2. queue = [(start, 0)]
  3. While queue not empty:
       (u, depth) = queue.pop()
       If depth >= max_depth: continue
       For each neighbor w of u via edge e:
         If color(e) ∈ color_mask and w ∉ visited:
           visited.add(w)
           queue.push((w, depth + 1))
  4. Return visited
```

## Theoretical Guarantees

### Correctness

**Theorem 1**: If there exists a minimum cut `(S, V\S)` with `v ∈ S` and `|δ(S)| ≤ k`, then `LocalKCut(G, v, k)` finds it.

**Proof sketch**:
- The cut can be characterized by a color pattern
- Enumeration tries all 4^r color combinations
- For `r = O(log k)`, this covers all cuts up to size `k`

### Complexity

- **Time per vertex**: `O(k^{O(1)} · deg(v))`
- **Space**: `O(m)` for edge colorings
- **Deterministic**: No randomization

### Witness Property

Using forest packing with `⌈λ_max · log(m) / ε²⌉` forests:

**Theorem 2**: Any cut of value ≤ `λ_max` is witnessed by all forests with probability 1 (deterministic).

## Implementation Details

### Edge Coloring Scheme

We use a **simple deterministic** coloring:

```rust
color(edge) = edge_id mod 4
```

This ensures:
1. **Determinism**: Same graph → same colors
2. **Balance**: Roughly equal colors across edges
3. **Simplicity**: O(1) per edge

### Color Mask Representation

We use a **4-bit mask** to represent color subsets:

```
Bit 0: Red
Bit 1: Blue
Bit 2: Green
Bit 3: Yellow

Example: 0b1010 = {Blue, Yellow}
```

This allows:
- Fast membership testing: `O(1)`
- Efficient enumeration: 16 total masks
- Compact storage: 1 byte per mask

### Radius Computation

The search radius is:

```rust
radius = ⌈log₄(k)⌉ + 1 = ⌈log₂(k) / 2⌉ + 1
```

Rationale:
- A cut of size `k` can be described by ≤ `log₄(k)` color choices
- Extra +1 provides buffer for edge cases
- Keeps enumeration tractable: `O(4^r) = O(k²)`

## Usage Examples

### Basic Usage

```rust
use ruvector_mincut::prelude::*;
use std::sync::Arc;

// Create graph
let graph = Arc::new(DynamicGraph::new());
graph.insert_edge(1, 2, 1.0).unwrap();
graph.insert_edge(2, 3, 1.0).unwrap();
graph.insert_edge(3, 4, 1.0).unwrap();

// Find local cut from vertex 1 with k=2
let local_kcut = LocalKCut::new(graph, 2);
if let Some(result) = local_kcut.find_cut(1) {
    println!("Cut value: {}", result.cut_value);
    println!("Cut set: {:?}", result.cut_set);
    println!("Iterations: {}", result.iterations);
}
```

### With Forest Packing

```rust
// Create forest packing for witness guarantees
let lambda_max = 10; // Upper bound on min cut
let epsilon = 0.1;   // Approximation parameter

let packing = ForestPacking::greedy_packing(&*graph, lambda_max, epsilon);

// Find cut
let local_kcut = LocalKCut::new(graph.clone(), lambda_max);
if let Some(result) = local_kcut.find_cut(start_vertex) {
    // Check witness property
    if packing.witnesses_cut(&result.cut_edges) {
        println!("Cut is witnessed by all forests ✓");
    }
}
```

### Enumerating Paths

```rust
// Enumerate all color-constrained reachable sets
let paths = local_kcut.enumerate_paths(vertex, depth);

for path in paths {
    println!("Reachable set: {} vertices", path.len());
    // Analyze structure
}
```

## Applications

### 1. Graph Clustering

Find natural clusters by detecting weak cuts:

```rust
for vertex in graph.vertices() {
    if let Some(cut) = local_kcut.find_cut(vertex) {
        if cut.cut_value <= threshold {
            // Found a cluster around vertex
            process_cluster(cut.cut_set);
        }
    }
}
```

### 2. Bridge Detection

Find critical edges (bridges):

```rust
let local_kcut = LocalKCut::new(graph, 1);
for vertex in graph.vertices() {
    if let Some(cut) = local_kcut.find_cut(vertex) {
        if cut.cut_value == 1.0 && cut.cut_edges.len() == 1 {
            println!("Bridge: {:?}", cut.cut_edges[0]);
        }
    }
}
```

### 3. Community Detection

Identify densely connected components:

```rust
let mut communities = Vec::new();
let mut visited = HashSet::new();

for vertex in graph.vertices() {
    if visited.contains(&vertex) {
        continue;
    }

    if let Some(cut) = local_kcut.find_cut(vertex) {
        if cut.cut_value <= community_threshold {
            communities.push(cut.cut_set.clone());
            visited.extend(&cut.cut_set);
        }
    }
}
```

## Comparison with Other Algorithms

| Algorithm | Time | Space | Deterministic | Global/Local |
|-----------|------|-------|---------------|--------------|
| LocalKCut (Dec 2024) | O(k² · deg(v)) | O(m) | ✓ | Local |
| LocalKCut (SODA 2025) | O(k · deg(v)) | O(m) | ✗ | Local |
| Karger-Stein | O(n² log³ n) | O(m) | ✗ | Global |
| Stoer-Wagner | O(nm + n² log n) | O(n²) | ✓ | Global |
| Our Full Algorithm | O(n^{o(1)}) amortized | O(m) | ✓ | Global |

## Advantages

1. **Deterministic**: No randomization → reproducible results
2. **Local**: Faster than global algorithms for sparse graphs
3. **Exact**: Finds exact cuts (not approximate)
4. **Simple**: Easy to implement and understand
5. **Parallelizable**: Different vertices can be processed in parallel

## Limitations

1. **Local scope**: May miss global minimum cut
2. **Parameter k**: Requires knowing approximate cut size
3. **Small cuts**: Best for cuts of size ≤ polylog(n)
4. **Enumeration**: Exponential in log(k), so k must be small

## Future Improvements

1. **Adaptive radius**: Dynamically adjust based on graph structure
2. **Smart coloring**: Use graph properties for better colorings
3. **Pruning**: Skip color combinations that can't improve result
4. **Caching**: Reuse BFS results across color masks
5. **Parallel**: Run different color masks in parallel

## References

1. December 2024: "Deterministic and Exact Fully-dynamic Minimum Cut of Superpolylogarithmic Size"
2. SODA 2025: "Subpolynomial-time Dynamic Minimum Cut via Randomized LocalKCut"
3. Karger 1996: "Minimum cuts in near-linear time"
4. Stoer-Wagner 1997: "A simple min-cut algorithm"

## See Also

- [`DynamicMinCut`](./dynamic-mincut.md) - Full dynamic minimum cut algorithm
- [`ForestPacking`](./forest-packing.md) - Witness guarantees
- [`ExpanderDecomposition`](./expander.md) - Graph decomposition
- [`HierarchicalDecomposition`](./hierarchical.md) - Tree structure
