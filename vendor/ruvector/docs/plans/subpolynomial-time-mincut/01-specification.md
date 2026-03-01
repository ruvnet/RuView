# SPARC Phase 1: Specification - Subpolynomial-Time Dynamic Minimum Cut

## Executive Summary

This specification defines a **deterministic, fully-dynamic minimum-cut algorithm** with amortized update time **n^{o(1)}** (subpolynomial growth), achieving breakthrough performance for real-time graph monitoring. The system handles minimum cuts up to **2^{Θ((log n)^{3/4})}** edges and provides **(1+ε)-approximate** cuts via sparsification.

## 1. Problem Statement

### 1.1 Core Challenge
Maintain the minimum cut of a dynamic undirected graph under edge insertions and deletions with:
- **Subpolynomial amortized update time**: O(n^{o(1)}) per operation
- **Deterministic guarantees**: No probabilistic error
- **Real-time monitoring**: Hundreds to thousands of updates/second
- **Exact and approximate modes**: Exact for small cuts, (1+ε)-approximate for general graphs

### 1.2 Theoretical Foundation
Based on recent breakthrough work (Abboud et al., 2021+) that achieves:
- **Exact minimum cut**: For cuts of size ≤ 2^{Θ((log n)^{3/4})}
- **Approximate (1+ε) minimum cut**: For arbitrary cuts via sparsification
- **Subpolynomial complexity**: Breaking the O(√n) barrier of previous work (Thorup 2007)

### 1.3 Performance Baselines
- **Static computation**:
  - Stoer–Wagner: O(mn + n² log n) ≈ O(n³) for dense graphs
  - Karger's randomized: O(n² log³ n)
- **Prior dynamic (approximate)**:
  - Thorup: O(√n) amortized per update
- **Target**:
  - **n^{o(1)}** amortized (e.g., O(n^{0.1}) or O(polylog n))
  - P95 latency: <10ms for graphs with n=10,000
  - Throughput: 1,000-10,000 updates/second

## 2. Requirements

### 2.1 Functional Requirements

#### FR-1: Dynamic Graph Operations
- **FR-1.1**: Insert edge between two vertices in O(n^{o(1)}) amortized time
- **FR-1.2**: Delete edge between two vertices in O(n^{o(1)}) amortized time
- **FR-1.3**: Query current minimum cut value in O(1) time
- **FR-1.4**: Retrieve minimum cut partition in O(k) time where k is cut size

#### FR-2: Cut Maintenance
- **FR-2.1**: Maintain exact minimum cut for cuts ≤ 2^{Θ((log n)^{3/4})}
- **FR-2.2**: Provide (1+ε)-approximate minimum cut for larger cuts
- **FR-2.3**: Support configurable ε parameter (default: 0.01)
- **FR-2.4**: Track edge connectivity between all vertex pairs

#### FR-3: Data Structures
- **FR-3.1**: Hierarchical tree decomposition for cut maintenance
- **FR-3.2**: Link-cut trees for dynamic tree operations
- **FR-3.3**: Euler tour trees for subtree queries
- **FR-3.4**: Graph sparsification via sampling

#### FR-4: Monitoring & Observability
- **FR-4.1**: Real-time metrics: current cut value, update count, tree depth
- **FR-4.2**: Performance tracking: P50/P95/P99 latency, throughput
- **FR-4.3**: Change notifications via callback mechanism
- **FR-4.4**: Historical cut value tracking

### 2.2 Non-Functional Requirements

#### NFR-1: Performance
- **NFR-1.1**: Amortized update time: O(n^{o(1)})
- **NFR-1.2**: Query time: O(1) for cut value, O(k) for cut partition
- **NFR-1.3**: Memory usage: O(m + n) where m is edge count
- **NFR-1.4**: Throughput: ≥1,000 updates/second for n=10,000

#### NFR-2: Correctness
- **NFR-2.1**: Deterministic: No probabilistic error in results
- **NFR-2.2**: Exact: For cuts ≤ 2^{Θ((log n)^{3/4})}
- **NFR-2.3**: Bounded approximation: (1+ε) guarantee for larger cuts
- **NFR-2.4**: Consistency: All queries return current state

#### NFR-3: Scalability
- **NFR-3.1**: Handle graphs up to 100,000 vertices
- **NFR-3.2**: Support millions of edges
- **NFR-3.3**: Graceful degradation for large cuts
- **NFR-3.4**: Parallel update processing (future enhancement)

#### NFR-4: Integration
- **NFR-4.1**: Rust API compatible with ruvector-graph
- **NFR-4.2**: Zero-copy integration where possible
- **NFR-4.3**: C ABI for foreign function interface
- **NFR-4.4**: Thread-safe for concurrent queries

## 3. API Design

### 3.1 Core Types

```rust
/// Dynamic minimum cut data structure
pub struct DynamicMinCut {
    // Internal state (opaque)
}

/// Cut result
#[derive(Debug, Clone)]
pub struct MinCutResult {
    pub value: usize,
    pub partition_a: Vec<VertexId>,
    pub partition_b: Vec<VertexId>,
    pub cut_edges: Vec<(VertexId, VertexId)>,
    pub is_exact: bool,
    pub epsilon: Option<f64>,
}

/// Configuration
#[derive(Debug, Clone)]
pub struct MinCutConfig {
    pub epsilon: f64,              // Approximation factor (default: 0.01)
    pub max_exact_cut_size: usize, // Threshold for exact vs approximate
    pub enable_monitoring: bool,   // Track performance metrics
    pub use_sparsification: bool,  // Enable graph sparsification
}

/// Performance metrics
#[derive(Debug, Clone)]
pub struct MinCutMetrics {
    pub current_cut_value: usize,
    pub update_count: u64,
    pub tree_depth: usize,
    pub graph_size: (usize, usize), // (vertices, edges)
    pub avg_update_time_ns: u64,
    pub p95_update_time_ns: u64,
    pub p99_update_time_ns: u64,
}
```

### 3.2 Primary API

```rust
impl DynamicMinCut {
    /// Create new dynamic min-cut structure for graph
    pub fn new(config: MinCutConfig) -> Self;

    /// Initialize from existing graph
    pub fn from_graph(graph: &Graph, config: MinCutConfig) -> Self;

    /// Insert edge (u, v)
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId) -> Result<()>;

    /// Delete edge (u, v)
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) -> Result<()>;

    /// Get current minimum cut value in O(1)
    pub fn min_cut_value(&self) -> usize;

    /// Get minimum cut partition in O(k) where k is cut size
    pub fn min_cut(&self) -> MinCutResult;

    /// Check connectivity between two vertices
    pub fn edge_connectivity(&self, u: VertexId, v: VertexId) -> usize;

    /// Get performance metrics
    pub fn metrics(&self) -> MinCutMetrics;

    /// Reset to empty graph
    pub fn clear(&mut self);
}
```

### 3.3 Monitoring API

```rust
/// Callback for cut value changes
pub type CutChangeCallback = Box<dyn Fn(usize, usize) + Send + Sync>;

impl DynamicMinCut {
    /// Register callback for cut value changes
    pub fn on_cut_change(&mut self, callback: CutChangeCallback);

    /// Get historical cut values (if monitoring enabled)
    pub fn cut_history(&self) -> Vec<(Timestamp, usize)>;

    /// Export metrics for external monitoring
    pub fn export_metrics(&self) -> serde_json::Value;
}
```

### 3.4 Advanced API

```rust
impl DynamicMinCut {
    /// Batch insert edges for better amortization
    pub fn insert_edges(&mut self, edges: &[(VertexId, VertexId)]) -> Result<()>;

    /// Batch delete edges
    pub fn delete_edges(&mut self, edges: &[(VertexId, VertexId)]) -> Result<()>;

    /// Force recomputation (for validation/debugging)
    pub fn recompute(&mut self) -> MinCutResult;

    /// Get internal tree structure (for visualization)
    pub fn tree_structure(&self) -> TreeStructure;

    /// Validate internal consistency (debug builds only)
    #[cfg(debug_assertions)]
    pub fn validate(&self) -> Result<()>;
}
```

## 4. Algorithmic Specifications

### 4.1 Hierarchical Tree Decomposition

The core data structure maintains a **hierarchical decomposition tree** where:
- Each node represents a subset of vertices
- Leaf nodes are individual vertices
- Internal nodes represent contracted subgraphs
- Tree height is O(log n)
- Each level maintains cut information

**Properties**:
- **Invariant 1**: Minimum cut crosses at most one tree edge per level
- **Invariant 2**: Tree depth is O(log n)
- **Invariant 3**: Each node stores local minimum cut

### 4.2 Sparsification

For (1+ε)-approximate cuts:
- Sample edges with probability p ∝ 1/(ε²λ) where λ is minimum cut
- Maintain sparse graph H with O(n log n / ε²) edges
- Run dynamic algorithm on H instead of original graph
- Guarantee: (1-ε)λ ≤ λ_H ≤ (1+ε)λ

### 4.3 Link-Cut Trees

Used for:
- Maintaining spanning forest
- LCA (Lowest Common Ancestor) queries in O(log n)
- Path queries and updates
- Dynamic tree connectivity

### 4.4 Update Operations

#### Edge Insertion
1. Check if edge is tree or non-tree edge
2. If increases cut → update hierarchy O(log n) levels
3. Use link-cut tree for efficient path queries
4. Amortized cost: O(n^{o(1)})

#### Edge Deletion
1. If tree edge → find replacement edge
2. If non-tree edge → check if affects cut
3. Rebuild affected subtrees using Euler tour
4. Amortized cost: O(n^{o(1)})

## 5. Constraints & Assumptions

### 5.1 Graph Constraints
- **Undirected simple graphs**: No self-loops, no multi-edges
- **Connected graphs**: Algorithm maintains connectivity info
- **Vertex IDs**: Consecutive integers 0..n-1 (can map from arbitrary)
- **Edge weights**: Currently unweighted (extension possible)

### 5.2 Complexity Constraints
- **Exact mode**: Cut size ≤ 2^{Θ((log n)^{3/4})}
- **Approximate mode**: No size limit, uses sparsification
- **Memory**: O(m + n) total space
- **Update sequence**: Arbitrary insert/delete sequence

### 5.3 Implementation Constraints
- **Single-threaded core**: Lock-free reads, mutable writes
- **No unsafe code**: Except for verified performance-critical sections
- **No external dependencies**: For core algorithm (monitoring optional)
- **Cross-platform**: Pure Rust, no platform-specific code

## 6. Success Criteria

### 6.1 Correctness
- ✓ All exact cuts verified against Stoer-Wagner
- ✓ Approximate cuts within (1+ε) of optimal
- ✓ No false positives/negatives in connectivity queries
- ✓ Passes 10,000+ randomized test cases

### 6.2 Performance
- ✓ Update time: O(n^{0.2}) or better (measured empirically)
- ✓ Throughput: >1,000 updates/sec for n=10,000
- ✓ P95 latency: <10ms for n=10,000
- ✓ Memory overhead: <2x graph size

### 6.3 Integration
- ✓ Compiles as ruvector-mincut crate
- ✓ Integrates with ruvector-graph
- ✓ C ABI exports working
- ✓ Documentation with examples

## 7. Out of Scope (V1)

- Weighted graphs (future: weighted minimum cut)
- Directed graphs (different problem)
- Parallel update processing (future: concurrent updates)
- Distributed computation (future: partitioned graphs)
- GPU acceleration (research needed)

## 8. Dependencies

### 8.1 Internal Dependencies
- `ruvector-graph`: Graph data structures
- `ruvector-core`: Core utilities

### 8.2 External Dependencies (Minimal)
- `thiserror`: Error handling
- `serde`: Serialization (optional, for monitoring)
- `criterion`: Benchmarking (dev dependency)

## 9. Risk Analysis

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Complexity explosion for large cuts | Medium | High | Automatic fallback to approximate mode |
| Memory overhead | Low | Medium | Lazy allocation, sparsification |
| Integration challenges | Low | Low | Early prototyping with ruvector-graph |
| Performance regression | Low | High | Comprehensive benchmarking suite |

## 10. Acceptance Tests

### AT-1: Basic Operations
```rust
let mut mincut = DynamicMinCut::new(MinCutConfig::default());
mincut.insert_edge(0, 1);
mincut.insert_edge(1, 2);
mincut.insert_edge(2, 3);
assert_eq!(mincut.min_cut_value(), 1); // Bridge at (1,2) or (2,3)
```

### AT-2: Dynamic Updates
```rust
// Start with complete graph K4
let mut mincut = DynamicMinCut::from_complete_graph(4);
assert_eq!(mincut.min_cut_value(), 3); // Any single vertex

// Remove edges to create bottleneck
mincut.delete_edge(0, 2);
mincut.delete_edge(0, 3);
mincut.delete_edge(1, 2);
mincut.delete_edge(1, 3);
assert_eq!(mincut.min_cut_value(), 1); // Cut between {0,1} and {2,3}
```

### AT-3: Performance Target
```rust
let mut mincut = DynamicMinCut::new(MinCutConfig::default());
let graph = generate_random_graph(10_000, 50_000);
mincut = DynamicMinCut::from_graph(&graph, config);

let updates = generate_random_updates(10_000);
let start = Instant::now();
for (u, v, is_insert) in updates {
    if is_insert {
        mincut.insert_edge(u, v);
    } else {
        mincut.delete_edge(u, v);
    }
}
let duration = start.elapsed();
assert!(duration.as_secs_f64() < 10.0); // <10s for 10K updates
```

## 11. Documentation Requirements

- **API documentation**: 100% coverage with examples
- **Algorithm explanation**: Detailed markdown with diagrams
- **Performance guide**: When to use exact vs approximate
- **Integration guide**: Examples with ruvector-graph
- **Benchmarking guide**: How to measure and compare

## 12. Timeline Estimate

- **Phase 1 (Specification)**: 2 days - CURRENT
- **Phase 2 (Pseudocode)**: 3 days - Algorithm design
- **Phase 3 (Architecture)**: 2 days - Module structure
- **Phase 4 (Refinement/TDD)**: 10 days - Implementation + testing
- **Phase 5 (Completion)**: 3 days - Integration + documentation

**Total**: ~20 days for V1 release

---

**Next Phase**: Proceed to `02-pseudocode.md` for detailed algorithm design.
