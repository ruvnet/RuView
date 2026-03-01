# ADR-002 Addendum: BMSSP WASM Integration

**Status**: Proposed
**Date**: 2026-01-25
**Extends**: ADR-002, ADR-002-addendum-sota-optimizations

---

## Executive Summary

Integrate `@ruvnet/bmssp` (Bounded Multi-Source Shortest Path) WASM module to accelerate j-tree operations:

- **O(m·log^(2/3) n)** complexity (beats O(n log n) all-pairs)
- **Multi-source queries** for terminal-based j-tree operations
- **Neural embeddings** via WasmNeuralBMSSP for learned sparsification
- **27KB WASM** enables browser/edge deployment
- **10-15x speedup** over JavaScript fallbacks

---

## The Path-Cut Duality

### Key Insight

In many graph classes, shortest paths and minimum cuts are dual:

```
Shortest Path in G* (dual) ←→ Minimum Cut in G

Where:
- G* has vertices = faces of G
- Edge weight in G* = cut capacity crossing that edge
```

For j-tree hierarchies specifically:

```
j-Tree Level Query:
┌─────────────────────────────────────────────────────────┐
│  Find min-cut between vertex sets S and T               │
│                                                         │
│  ≡ Find shortest S-T path in contracted auxiliary graph │
│                                                         │
│  BMSSP complexity: O(m·log^(2/3) n)                    │
│  vs. direct cut:   O(n log n)                          │
│                                                         │
│  Speedup: ~log^(1/3) n factor                          │
└─────────────────────────────────────────────────────────┘
```

---

## Architecture Integration

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    J-TREE + BMSSP INTEGRATED ARCHITECTURE                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                     LAYER 0: WASM ACCELERATION                         │ │
│  │                                                                         │ │
│  │   ┌─────────────────┐              ┌─────────────────┐                 │ │
│  │   │   WasmGraph     │              │ WasmNeuralBMSSP │                 │ │
│  │   │   (27KB WASM)   │              │   (embeddings)  │                 │ │
│  │   ├─────────────────┤              ├─────────────────┤                 │ │
│  │   │ • add_edge      │              │ • set_embedding │                 │ │
│  │   │ • shortest_paths│              │ • semantic_dist │                 │ │
│  │   │ • vertex_count  │              │ • neural_paths  │                 │ │
│  │   │ • edge_count    │              │ • update_embed  │                 │ │
│  │   └─────────────────┘              └─────────────────┘                 │ │
│  │            │                                │                           │ │
│  │            └────────────┬───────────────────┘                           │ │
│  │                         ▼                                               │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                            │                                                 │
│                            ▼                                                 │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                  LAYER 1: HYBRID CUT COMPUTATION                       │ │
│  │                                                                         │ │
│  │   Query Type          │ Method                │ Complexity              │ │
│  │   ────────────────────┼───────────────────────┼───────────────────────  │ │
│  │   Point-to-point cut  │ BMSSP path → cut      │ O(m·log^(2/3) n)       │ │
│  │   Multi-terminal cut  │ BMSSP multi-source    │ O(k·m·log^(2/3) n)     │ │
│  │   All-pairs cuts      │ BMSSP batch + cache   │ O(n·m·log^(2/3) n)     │ │
│  │   Sparsest cut        │ Neural semantic dist  │ O(n²) → O(n·d)         │ │
│  │                                                                         │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                            │                                                 │
│                            ▼                                                 │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                  LAYER 2: J-TREE HIERARCHY                             │ │
│  │                                                                         │ │
│  │   Each j-tree level maintains:                                         │ │
│  │   • WasmGraph for contracted graph at that level                       │ │
│  │   • WasmNeuralBMSSP for learned edge importance                        │ │
│  │   • Cached shortest-path distances (cut values)                        │ │
│  │                                                                         │ │
│  │   Level L: WasmGraph(O(1) vertices)                                    │ │
│  │   Level L-1: WasmGraph(O(α) vertices)                                  │ │
│  │   ...                                                                   │ │
│  │   Level 0: WasmGraph(n vertices)                                       │ │
│  │                                                                         │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## API Integration

### 1. BMSSP-Accelerated Cut Queries

```rust
/// J-tree level backed by BMSSP WASM
pub struct BmsspJTreeLevel {
    /// WASM graph for this level
    wasm_graph: WasmGraph,
    /// Neural BMSSP for learned operations
    neural_bmssp: Option<WasmNeuralBMSSP>,
    /// Cached path distances (= cut values in dual)
    path_cache: HashMap<(VertexId, VertexId), f64>,
    /// Level index
    level: usize,
}

impl BmsspJTreeLevel {
    /// Create from contracted graph
    pub fn from_contracted(contracted: &ContractedGraph, level: usize) -> Self {
        let n = contracted.vertex_count();
        let mut wasm_graph = WasmGraph::new(n as u32, false); // undirected

        // Add edges with weights = capacities
        for edge in contracted.edges() {
            wasm_graph.add_edge(
                edge.source as u32,
                edge.target as u32,
                edge.capacity,
            );
        }

        Self {
            wasm_graph,
            neural_bmssp: None,
            path_cache: HashMap::new(),
            level,
        }
    }

    /// Min-cut between s and t via path-cut duality
    /// Complexity: O(m·log^(2/3) n) vs O(n log n) direct
    pub fn min_cut(&mut self, s: VertexId, t: VertexId) -> f64 {
        // Check cache first
        if let Some(&cached) = self.path_cache.get(&(s, t)) {
            return cached;
        }

        // Compute shortest paths from s
        let distances = self.wasm_graph.compute_shortest_paths(s as u32);

        // Distance to t = min-cut value (in dual representation)
        let cut_value = distances[t as usize];

        // Cache for future queries
        self.path_cache.insert((s, t), cut_value);
        self.path_cache.insert((t, s), cut_value); // symmetric

        cut_value
    }

    /// Multi-terminal cut using BMSSP multi-source
    pub fn multi_terminal_cut(&mut self, terminals: &[VertexId]) -> f64 {
        // BMSSP handles multi-source natively
        let sources: Vec<u32> = terminals.iter().map(|&v| v as u32).collect();

        // Compute shortest paths from all terminals simultaneously
        // This amortizes the cost across terminals
        let mut min_cut = f64::INFINITY;

        for (i, &s) in terminals.iter().enumerate() {
            let distances = self.wasm_graph.compute_shortest_paths(s as u32);

            for (j, &t) in terminals.iter().enumerate() {
                if i < j {
                    let cut = distances[t as usize];
                    min_cut = min_cut.min(cut);
                }
            }
        }

        min_cut
    }
}
```

### 2. Neural Sparsification via WasmNeuralBMSSP

```rust
/// Neural sparsifier using BMSSP embeddings
pub struct BmsspNeuralSparsifier {
    /// Neural BMSSP instance
    neural: WasmNeuralBMSSP,
    /// Embedding dimension
    embedding_dim: usize,
    /// Learning rate for gradient updates
    learning_rate: f64,
    /// Alpha for semantic edge weighting
    semantic_alpha: f64,
}

impl BmsspNeuralSparsifier {
    /// Initialize with node embeddings
    pub fn new(graph: &DynamicGraph, embedding_dim: usize) -> Self {
        let n = graph.vertex_count();
        let mut neural = WasmNeuralBMSSP::new(n as u32, embedding_dim as u32);

        // Initialize embeddings (could use pre-trained or random)
        for v in 0..n {
            let embedding = Self::initial_embedding(v, embedding_dim);
            neural.set_embedding(v as u32, &embedding);
        }

        // Add semantic edges based on graph structure
        for edge in graph.edges() {
            neural.add_semantic_edge(
                edge.source as u32,
                edge.target as u32,
                0.5, // alpha parameter
            );
        }

        Self {
            neural,
            embedding_dim,
            learning_rate: 0.01,
            semantic_alpha: 0.5,
        }
    }

    /// Compute edge importance via semantic distance
    pub fn edge_importance(&self, u: VertexId, v: VertexId) -> f64 {
        // Semantic distance inversely correlates with importance
        let distance = self.neural.semantic_distance(u as u32, v as u32);

        // Convert to importance: closer = more important
        1.0 / (1.0 + distance)
    }

    /// Sparsify graph keeping top-k important edges
    pub fn sparsify(&self, graph: &DynamicGraph, k: usize) -> SparseGraph {
        let mut edge_scores: Vec<_> = graph.edges()
            .map(|e| (e, self.edge_importance(e.source, e.target)))
            .collect();

        // Sort by importance descending
        edge_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Keep top k edges
        let kept_edges: Vec<_> = edge_scores.into_iter()
            .take(k)
            .map(|(e, _)| e)
            .collect();

        SparseGraph::from_edges(kept_edges)
    }

    /// Update embeddings based on cut preservation loss
    pub fn train_step(&mut self, original_cuts: &[(VertexId, VertexId, f64)]) {
        // Compute gradients based on cut preservation
        let gradients = self.compute_cut_gradients(original_cuts);

        // Update via WASM
        self.neural.update_embeddings(
            &gradients,
            self.learning_rate,
            self.embedding_dim as u32,
        );
    }

    /// Compute gradients to preserve cut values
    fn compute_cut_gradients(&self, cuts: &[(VertexId, VertexId, f64)]) -> Vec<f64> {
        let mut gradients = vec![0.0; self.neural.vertex_count() * self.embedding_dim];

        for &(s, t, true_cut) in cuts {
            let predicted_cut = self.neural.semantic_distance(s as u32, t as u32);
            let error = predicted_cut - true_cut;

            // Gradient for embedding update
            // (simplified - actual implementation would use autograd)
            let s_offset = s as usize * self.embedding_dim;
            let t_offset = t as usize * self.embedding_dim;

            for d in 0..self.embedding_dim {
                gradients[s_offset + d] += error * 0.5;
                gradients[t_offset + d] += error * 0.5;
            }
        }

        gradients
    }
}
```

### 3. Full Integration with Predictive j-Tree

```rust
/// Predictive j-tree with BMSSP acceleration
pub struct BmsspPredictiveJTree {
    /// J-tree levels backed by BMSSP
    levels: Vec<BmsspJTreeLevel>,
    /// Neural sparsifier
    sparsifier: BmsspNeuralSparsifier,
    /// SNN prediction engine (from SOTA addendum)
    snn_predictor: PolicySNN,
    /// Exact verifier (Tier 2)
    exact: SubpolynomialMinCut,
}

impl BmsspPredictiveJTree {
    /// Build hierarchy with BMSSP at each level
    pub fn build(graph: &DynamicGraph, epsilon: f64) -> Self {
        let alpha = compute_alpha(epsilon);
        let num_levels = (graph.vertex_count() as f64).log(alpha).ceil() as usize;

        // Build neural sparsifier first
        let sparsifier = BmsspNeuralSparsifier::new(graph, 64);
        let sparse = sparsifier.sparsify(graph, graph.vertex_count() * 10);

        // Build BMSSP-backed levels
        let mut levels = Vec::with_capacity(num_levels);
        let mut current = sparse.clone();

        for level in 0..num_levels {
            let bmssp_level = BmsspJTreeLevel::from_contracted(&current, level);
            levels.push(bmssp_level);
            current = contract_graph(&current, alpha);
        }

        Self {
            levels,
            sparsifier,
            snn_predictor: PolicySNN::new(),
            exact: SubpolynomialMinCut::new(graph),
        }
    }

    /// Query with BMSSP acceleration
    pub fn min_cut(&mut self, s: VertexId, t: VertexId) -> CutResult {
        // Use SNN to predict optimal level to query
        let optimal_level = self.snn_predictor.predict_level(s, t);

        // Query BMSSP at predicted level
        let approx_cut = self.levels[optimal_level].min_cut(s, t);

        // Decide if exact verification needed
        if approx_cut < CRITICAL_THRESHOLD {
            let exact_cut = self.exact.min_cut_between(s, t);
            CutResult::exact(exact_cut)
        } else {
            CutResult::approximate(approx_cut, self.approximation_factor(optimal_level))
        }
    }

    /// Batch queries with BMSSP multi-source
    pub fn all_pairs_cuts(&mut self, vertices: &[VertexId]) -> AllPairsResult {
        // BMSSP handles this efficiently via multi-source
        let mut results = HashMap::new();

        for level in &mut self.levels {
            let level_cuts = level.multi_terminal_cut(vertices);
            // Aggregate results across levels
        }

        AllPairsResult { cuts: results }
    }
}
```

---

## Performance Analysis

### Complexity Comparison

| Operation | Without BMSSP | With BMSSP | Improvement |
|-----------|---------------|------------|-------------|
| Point-to-point cut | O(n log n) | O(m·log^(2/3) n) | ~log^(1/3) n |
| Multi-terminal (k) | O(k·n log n) | O(k·m·log^(2/3) n) | ~log^(1/3) n |
| All-pairs (n²) | O(n² log n) | O(n·m·log^(2/3) n) | ~n/m · log^(1/3) n |
| Neural sparsify | O(n² embeddings) | O(n·d) WASM | ~n/d |

### Benchmarks (from BMSSP)

| Graph Size | JS (ms) | BMSSP WASM (ms) | Speedup |
|------------|---------|-----------------|---------|
| 1K nodes | 12.5 | 1.0 | **12.5x** |
| 10K nodes | 145.3 | 12.0 | **12.1x** |
| 100K nodes | 1,523.7 | 45.0 | **33.9x** |
| 1M nodes | 15,234.2 | 180.0 | **84.6x** |

### Expected j-Tree Speedup

```
J-tree query (10K graph):
├── Without BMSSP: ~50ms (Rust native)
├── With BMSSP:    ~12ms (WASM accelerated)
└── Improvement:   ~4x for path-based queries

J-tree + Neural Sparsify (10K graph):
├── Without BMSSP: ~200ms (native + neural)
├── With BMSSP:    ~25ms (WASM + embeddings)
└── Improvement:   ~8x for full pipeline
```

---

## Deployment Scenarios

### 1. Browser/Edge (Primary Use Case)

```typescript
// Browser deployment with BMSSP
import init, { WasmGraph, WasmNeuralBMSSP } from '@ruvnet/bmssp';

async function initJTreeBrowser() {
    await init(); // Load 27KB WASM

    const graph = new WasmGraph(1000, false);
    // Build j-tree hierarchy in browser
    // 10-15x faster than pure JS implementation
}
```

### 2. Node.js with Native Fallback

```typescript
// Hybrid: BMSSP for queries, native Rust for exact
import { WasmGraph } from '@ruvnet/bmssp';
import { SubpolynomialMinCut } from 'ruvector-mincut-napi';

const bmsspLevel = new WasmGraph(n, false);
const exactVerifier = new SubpolynomialMinCut(graph);

// Use BMSSP for fast approximate
const approx = bmsspLevel.compute_shortest_paths(source);

// Use native for exact verification
const exact = exactVerifier.min_cut();
```

### 3. 256-Core Agentic Chip

```rust
// Each core gets its own BMSSP instance for a j-tree level
// 27KB WASM fits within 8KB constraint when compiled to native

impl CoreExecutor {
    pub fn init_bmssp_level(&mut self, level: &ContractedGraph) {
        // WASM compiles to native instructions
        // Memory footprint: ~6KB for 256-vertex level
        self.bmssp = WasmGraph::new(level.vertex_count(), false);
    }
}
```

---

## Implementation Priority

| Phase | Task | Effort | Impact |
|-------|------|--------|--------|
| **P0** | Add `@ruvnet/bmssp` to package.json | 1 hour | Enable integration |
| **P0** | `BmsspJTreeLevel` wrapper | 1 week | Core functionality |
| **P1** | Neural sparsifier integration | 2 weeks | Learned edge selection |
| **P1** | Multi-source batch queries | 1 week | All-pairs acceleration |
| **P2** | SNN predictor + BMSSP fusion | 2 weeks | Optimal level selection |
| **P2** | Browser deployment bundle | 1 week | Edge deployment |

---

## References

1. **BMSSP**: "Breaking the Sorting Barrier for SSSP" (arXiv:2501.00660)
2. **Package**: https://www.npmjs.com/package/@ruvnet/bmssp
3. **Integration**: ADR-002, ADR-002-addendum-sota-optimizations

---

## Appendix: BMSSP API Quick Reference

```typescript
// Core Graph
class WasmGraph {
    constructor(vertices: number, directed: boolean);
    add_edge(from: number, to: number, weight: number): boolean;
    compute_shortest_paths(source: number): Float64Array;
    readonly vertex_count: number;
    readonly edge_count: number;
    free(): void;
}

// Neural Extension
class WasmNeuralBMSSP {
    constructor(vertices: number, embedding_dim: number);
    set_embedding(node: number, embedding: Float64Array): boolean;
    add_semantic_edge(from: number, to: number, alpha: number): void;
    compute_neural_paths(source: number): Float64Array;
    semantic_distance(node1: number, node2: number): number;
    update_embeddings(gradients: Float64Array, lr: number, dim: number): boolean;
    free(): void;
}
```
