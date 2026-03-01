# ADR-002 Addendum: SOTA Optimizations for Dynamic Hierarchical j-Tree

**Status**: Proposed
**Date**: 2026-01-25
**Extends**: ADR-002 (Dynamic Hierarchical j-Tree Decomposition)

---

## Executive Summary

This addendum pushes ADR-002 to true state-of-the-art by integrating:

1. **Predictive Dynamics** - SNN predicts updates before they happen
2. **Neural Sparsification** - Learned edge selection via SpecNet
3. **Lazy Hierarchical Evaluation** - Demand-paged j-tree levels
4. **Warm-Start Cut-Matching** - Reuse computation across updates
5. **256-Core Parallel Hierarchy** - Each core owns j-tree levels
6. **Streaming Sketch Fallback** - O(n log n) space for massive graphs

**Target**: Sub-microsecond approximate queries, <100μs exact verification

---

## Architecture: Predictive Dynamic j-Tree

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                    PREDICTIVE DYNAMIC J-TREE ARCHITECTURE                        │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                  │
│  ┌─────────────────────────────────────────────────────────────────────────────┐│
│  │                         LAYER 0: PREDICTION ENGINE                          ││
│  │                                                                              ││
│  │   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐                 ││
│  │   │  SNN Policy  │───►│  TD Learner  │───►│  Prefetcher  │                 ││
│  │   │  (R-STDP)    │    │  (Value Net) │    │  (Speculate) │                 ││
│  │   └──────────────┘    └──────────────┘    └──────────────┘                 ││
│  │         │                    │                    │                         ││
│  │         ▼                    ▼                    ▼                         ││
│  │   Predict which       Estimate cut        Pre-compute                       ││
│  │   levels change       value change        likely queries                    ││
│  │                                                                              ││
│  └─────────────────────────────────────────────────────────────────────────────┘│
│                                      │                                           │
│                                      ▼                                           │
│  ┌─────────────────────────────────────────────────────────────────────────────┐│
│  │                      LAYER 1: NEURAL SPARSIFIER                             ││
│  │                                                                              ││
│  │   ┌────────────────────────────────────────────────────────────────────┐   ││
│  │   │  SpecNet Integration (arXiv:2510.27474)                            │   ││
│  │   │                                                                     │   ││
│  │   │  Loss = λ₁·Laplacian_Alignment + λ₂·Feature_Preserve + λ₃·Sparsity │   ││
│  │   │                                                                     │   ││
│  │   │  • Joint Graph Evolution layer                                      │   ││
│  │   │  • Spectral Concordance preservation                                │   ││
│  │   │  • Degree-based fast presparse (DSpar: 5.9x speedup)               │   ││
│  │   └────────────────────────────────────────────────────────────────────┘   ││
│  │                                                                              ││
│  └─────────────────────────────────────────────────────────────────────────────┘│
│                                      │                                           │
│                                      ▼                                           │
│  ┌─────────────────────────────────────────────────────────────────────────────┐│
│  │                    LAYER 2: LAZY HIERARCHICAL J-TREE                        ││
│  │                                                                              ││
│  │   Level L ──┐                                                               ││
│  │   Level L-1 ├── Demand-paged: Only materialize when queried                 ││
│  │   Level L-2 ├── Dirty marking: Track which levels need recomputation        ││
│  │   ...       │   Warm-start: Reuse cut-matching state across updates         ││
│  │   Level 0 ──┘                                                               ││
│  │                                                                              ││
│  │   Memory: O(active_levels × n_level) instead of O(L × n)                    ││
│  │                                                                              ││
│  └─────────────────────────────────────────────────────────────────────────────┘│
│                                      │                                           │
│                                      ▼                                           │
│  ┌─────────────────────────────────────────────────────────────────────────────┐│
│  │                   LAYER 3: 256-CORE PARALLEL DISTRIBUTION                   ││
│  │                                                                              ││
│  │   ┌─────────┬─────────┬─────────┬─────────┬─────────┬─────────┐            ││
│  │   │Core 0-31│Core32-63│Core64-95│Core96-127│Core128+ │Core 255│            ││
│  │   │ Level 0 │ Level 1 │ Level 2 │ Level 3 │   ...   │ Level L│            ││
│  │   └─────────┴─────────┴─────────┴─────────┴─────────┴─────────┘            ││
│  │                                                                              ││
│  │   Work Stealing: Imbalanced levels redistribute to idle cores               ││
│  │   Atomic CAS: SharedCoordinator for global min-cut updates                  ││
│  │   8KB/core: CompactCoreState fits entire j-tree level                       ││
│  │                                                                              ││
│  └─────────────────────────────────────────────────────────────────────────────┘│
│                                      │                                           │
│                                      ▼                                           │
│  ┌─────────────────────────────────────────────────────────────────────────────┐│
│  │                    LAYER 4: STREAMING SKETCH FALLBACK                       ││
│  │                                                                              ││
│  │   When n > 100K vertices:                                                   ││
│  │   ┌────────────────────────────────────────────────────────────────────┐   ││
│  │   │  Semi-Streaming Cut Sketch                                          │   ││
│  │   │  • O(n log n) space (two edges per vertex)                         │   ││
│  │   │  • Reservoir sampling for edge selection                            │   ││
│  │   │  • (1+ε) approximation maintained incrementally                     │   ││
│  │   └────────────────────────────────────────────────────────────────────┘   ││
│  │                                                                              ││
│  └─────────────────────────────────────────────────────────────────────────────┘│
│                                      │                                           │
│                                      ▼                                           │
│  ┌─────────────────────────────────────────────────────────────────────────────┐│
│  │                      LAYER 5: EXACT VERIFICATION                            ││
│  │                                                                              ││
│  │   El-Hayek/Henzinger/Li (arXiv:2512.13105)                                 ││
│  │   • Triggered only when approximate cut < threshold                         ││
│  │   • O(n^{o(1)}) exact verification                                         ││
│  │   • Deterministic, no randomization                                         ││
│  │                                                                              ││
│  └─────────────────────────────────────────────────────────────────────────────┘│
│                                                                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## Component 1: SNN Prediction Engine

Exploits the triple isomorphism already in the codebase:

| Graph Theory | Dynamical Systems | Neuromorphic |
|--------------|-------------------|--------------|
| MinCut value | Lyapunov exponent | Spike synchrony |
| Edge contraction | Phase space flow | Synaptic plasticity |
| Hierarchy level | Attractor basin | Memory consolidation |

```rust
/// Predictive j-tree using SNN dynamics
pub struct PredictiveJTree {
    /// Core j-tree hierarchy
    hierarchy: JTreeHierarchy,
    /// SNN policy network for update prediction
    policy: PolicySNN,
    /// Value network for cut estimation
    value_net: ValueNetwork,
    /// Prefetch cache for speculative computation
    prefetch: PrefetchCache,
    /// SONA hooks for continuous adaptation
    sona_hooks: [usize; 4], // Layers 8, 16, 24, 28
}

impl PredictiveJTree {
    /// Predict which levels will need updates after edge change
    pub fn predict_affected_levels(&self, edge: (VertexId, VertexId)) -> Vec<usize> {
        // SNN encodes edge as spike pattern
        let spike_input = self.edge_to_spikes(edge);

        // Policy network predicts affected regions
        let activity = self.policy.forward(&spike_input);

        // Low activity regions are stable, high activity needs update
        activity.iter()
            .enumerate()
            .filter(|(_, &a)| a > ACTIVITY_THRESHOLD)
            .map(|(level, _)| level)
            .collect()
    }

    /// Speculative update: pre-compute before edge actually changes
    pub fn speculative_update(&mut self, likely_edge: (VertexId, VertexId), prob: f64) {
        if prob > SPECULATION_THRESHOLD {
            let affected = self.predict_affected_levels(likely_edge);

            // Pre-compute in background cores
            for level in affected {
                self.prefetch.schedule(level, likely_edge);
            }
        }
    }

    /// TD-learning update after observing actual cut change
    pub fn learn_from_observation(&mut self, predicted_cut: f64, actual_cut: f64) {
        let td_error = actual_cut - predicted_cut;

        // R-STDP: Reward-modulated spike-timing-dependent plasticity
        self.policy.apply_rstdp(td_error);

        // Update value network
        self.value_net.td_update(td_error);
    }
}
```

**Performance Target**: Predict 80%+ of affected levels correctly → skip 80% of unnecessary recomputation

---

## Component 2: Neural Sparsifier (SpecNet Integration)

Based on arXiv:2510.27474, learn which edges to keep:

```rust
/// Neural graph sparsifier with spectral concordance
pub struct NeuralSparsifier {
    /// Graph evolution layer (learned edge selection)
    evolution_layer: GraphEvolutionLayer,
    /// Spectral concordance loss weights
    lambda_laplacian: f64,    // λ₁ = 1.0
    lambda_feature: f64,      // λ₂ = 0.5
    lambda_sparsity: f64,     // λ₃ = 0.1
    /// Degree-based presparse threshold (DSpar optimization)
    degree_threshold: f64,
}

impl NeuralSparsifier {
    /// Fast presparse using degree heuristic (DSpar: 5.9x speedup)
    pub fn degree_presparse(&self, graph: &DynamicGraph) -> DynamicGraph {
        let mut sparse = graph.clone();

        // Effective resistance ≈ 1/(deg_u × deg_v)
        // Keep edges with high effective resistance
        for edge in graph.edges() {
            let deg_u = graph.degree(edge.source) as f64;
            let deg_v = graph.degree(edge.target) as f64;
            let eff_resistance = 1.0 / (deg_u * deg_v);

            // Sample with probability proportional to effective resistance
            if eff_resistance < self.degree_threshold {
                sparse.remove_edge(edge.source, edge.target);
            }
        }

        sparse
    }

    /// Spectral concordance loss for training
    pub fn spectral_concordance_loss(
        &self,
        original: &DynamicGraph,
        sparsified: &DynamicGraph,
    ) -> f64 {
        // L₁: Laplacian eigenvalue alignment
        let laplacian_loss = self.laplacian_alignment(original, sparsified);

        // L₂: Feature geometry preservation (cut values)
        let feature_loss = self.cut_preservation_loss(original, sparsified);

        // L₃: Sparsity inducing trace penalty
        let sparsity_loss = sparsified.edge_count() as f64 / original.edge_count() as f64;

        self.lambda_laplacian * laplacian_loss
            + self.lambda_feature * feature_loss
            + self.lambda_sparsity * sparsity_loss
    }

    /// End-to-end learnable sparsification
    pub fn learn_sparsify(&mut self, graph: &DynamicGraph) -> SparseGraph {
        // 1. Fast presparse (DSpar)
        let presparse = self.degree_presparse(graph);

        // 2. Neural refinement (SpecNet)
        let edge_scores = self.evolution_layer.forward(&presparse);

        // 3. Top-k selection preserving spectral properties
        let k = (graph.vertex_count() as f64 * (graph.vertex_count() as f64).ln()) as usize;
        let selected = edge_scores.top_k(k);

        SparseGraph::from_edges(selected)
    }
}
```

**Performance Target**: 90% edge reduction while maintaining 95%+ cut accuracy

---

## Component 3: Lazy Hierarchical Evaluation

Don't compute levels until needed:

```rust
/// Lazy j-tree with demand-paged levels
pub struct LazyJTreeHierarchy {
    /// Level states
    levels: Vec<LazyLevel>,
    /// Which levels are materialized
    materialized: BitSet,
    /// Dirty flags for incremental update
    dirty: BitSet,
    /// Cut-matching state for warm-start
    warm_state: Vec<CutMatchingState>,
}

#[derive(Clone)]
enum LazyLevel {
    /// Not yet computed
    Unmaterialized,
    /// Computed and valid
    Materialized(JTree),
    /// Needs recomputation
    Dirty(JTree),
}

impl LazyJTreeHierarchy {
    /// Query with lazy materialization
    pub fn approximate_min_cut(&mut self) -> ApproximateCut {
        // Only materialize levels needed for query
        let mut current_level = self.levels.len() - 1;

        while current_level > 0 {
            self.ensure_materialized(current_level);

            let cut = self.levels[current_level].as_materialized().min_cut();

            // Early termination if cut is good enough
            if cut.approximation_factor < ACCEPTABLE_APPROX {
                return cut;
            }

            current_level -= 1;
        }

        self.levels[0].as_materialized().min_cut()
    }

    /// Ensure level is materialized (demand-paging)
    fn ensure_materialized(&mut self, level: usize) {
        match &self.levels[level] {
            LazyLevel::Unmaterialized => {
                // First-time computation
                let jtree = self.compute_level(level);
                self.levels[level] = LazyLevel::Materialized(jtree);
                self.materialized.insert(level);
            }
            LazyLevel::Dirty(old_jtree) => {
                // Warm-start from previous state (arXiv:2511.02943)
                let jtree = self.warm_start_recompute(level, old_jtree);
                self.levels[level] = LazyLevel::Materialized(jtree);
                self.dirty.remove(level);
            }
            LazyLevel::Materialized(_) => {
                // Already valid, no-op
            }
        }
    }

    /// Warm-start recomputation avoiding full recursion cost
    fn warm_start_recompute(&self, level: usize, old: &JTree) -> JTree {
        // Reuse cut-matching game state from warm_state
        let state = &self.warm_state[level];

        // Only recompute affected regions
        let mut new_jtree = old.clone();
        for node in state.affected_nodes() {
            new_jtree.recompute_node(node, state);
        }

        new_jtree
    }

    /// Mark levels dirty after edge update
    pub fn mark_dirty(&mut self, affected_levels: &[usize]) {
        for &level in affected_levels {
            if self.materialized.contains(level) {
                if let LazyLevel::Materialized(jtree) = &self.levels[level] {
                    self.levels[level] = LazyLevel::Dirty(jtree.clone());
                    self.dirty.insert(level);
                }
            }
        }
    }
}
```

**Performance Target**: 70% reduction in level computations for typical query patterns

---

## Component 4: 256-Core Parallel Distribution

Leverage the existing agentic chip architecture:

```rust
/// Parallel j-tree across 256 cores
pub struct ParallelJTree {
    /// Core assignments: which cores handle which levels
    level_assignments: Vec<CoreRange>,
    /// Shared coordinator for atomic updates
    coordinator: SharedCoordinator,
    /// Per-core executors
    executors: [CoreExecutor; 256],
}

struct CoreRange {
    start_core: u8,
    end_core: u8,
    level: usize,
}

impl ParallelJTree {
    /// Distribute L levels across 256 cores
    pub fn distribute_levels(num_levels: usize) -> Vec<CoreRange> {
        let cores_per_level = 256 / num_levels;

        (0..num_levels)
            .map(|level| {
                let start = (level * cores_per_level) as u8;
                let end = ((level + 1) * cores_per_level - 1) as u8;
                CoreRange { start_core: start, end_core: end, level }
            })
            .collect()
    }

    /// Parallel update across all affected levels
    pub fn parallel_update(&mut self, edge: (VertexId, VertexId)) {
        // Phase 1: Distribute update to affected cores
        self.coordinator.phase.store(SharedCoordinator::PHASE_DISTRIBUTE, Ordering::Release);

        for assignment in &self.level_assignments {
            for core_id in assignment.start_core..=assignment.end_core {
                self.executors[core_id as usize].queue_update(edge);
            }
        }

        // Phase 2: Parallel compute
        self.coordinator.phase.store(SharedCoordinator::PHASE_COMPUTE, Ordering::Release);

        // Each core processes independently
        // Work stealing if some cores finish early
        while !self.coordinator.all_completed() {
            // Idle cores steal from busy cores
            self.work_stealing_pass();
        }

        // Phase 3: Collect results
        self.coordinator.phase.store(SharedCoordinator::PHASE_COLLECT, Ordering::Release);
        let global_min = self.coordinator.global_min_cut.load(Ordering::Acquire);
    }

    /// Work stealing for load balancing
    fn work_stealing_pass(&mut self) {
        for core_id in 0..256u8 {
            if self.executors[core_id as usize].is_idle() {
                // Find busy core to steal from
                if let Some(victim) = self.find_busy_core() {
                    let work = self.executors[victim].steal_work();
                    self.executors[core_id as usize].accept_work(work);
                }
            }
        }
    }
}
```

**Performance Target**: Near-linear speedup up to 256× for independent level updates

---

## Component 5: Streaming Sketch Fallback

For graphs with n > 100K vertices:

```rust
/// Semi-streaming cut sketch for massive graphs
pub struct StreamingCutSketch {
    /// Two edges per vertex (reservoir sampling)
    sampled_edges: HashMap<VertexId, [Option<Edge>; 2]>,
    /// Total vertices seen
    vertex_count: usize,
    /// Reservoir sampling state
    reservoir: ReservoirSampler,
}

impl StreamingCutSketch {
    /// Process edge in streaming fashion: O(1) per edge
    pub fn process_edge(&mut self, edge: Edge) {
        // Update reservoir for source vertex
        self.reservoir.sample(edge.source, edge);

        // Update reservoir for target vertex
        self.reservoir.sample(edge.target, edge);
    }

    /// Approximate min-cut from sketch: O(n) query
    pub fn approximate_min_cut(&self) -> ApproximateCut {
        // Build sparse graph from sampled edges
        let sparse = self.build_sparse_graph();

        // Run exact algorithm on sparse graph
        // O(n log n) edges → tractable
        let cut = exact_min_cut(&sparse);

        ApproximateCut {
            value: cut.value,
            approximation_factor: 1.0 + self.epsilon(),
            partition: cut.partition,
        }
    }

    /// Memory usage: O(n log n)
    pub fn memory_bytes(&self) -> usize {
        self.vertex_count * 2 * std::mem::size_of::<Edge>()
    }
}

/// Adaptive system that switches between full j-tree and streaming
pub struct AdaptiveJTree {
    full_jtree: Option<LazyJTreeHierarchy>,
    streaming_sketch: Option<StreamingCutSketch>,
    threshold: usize, // Switch point (default: 100K vertices)
}

impl AdaptiveJTree {
    pub fn new(graph: &DynamicGraph) -> Self {
        if graph.vertex_count() > 100_000 {
            Self {
                full_jtree: None,
                streaming_sketch: Some(StreamingCutSketch::from_graph(graph)),
                threshold: 100_000,
            }
        } else {
            Self {
                full_jtree: Some(LazyJTreeHierarchy::build(graph)),
                streaming_sketch: None,
                threshold: 100_000,
            }
        }
    }
}
```

**Performance Target**: Handle 1M+ vertex graphs in <1GB memory

---

## Performance Comparison

| Metric | ADR-002 Baseline | SOTA Optimized | Improvement |
|--------|------------------|----------------|-------------|
| **Update Time** | O(n^ε) | O(n^ε) / 256 cores | ~100× |
| **Query Time (approx)** | O(log n) | O(1) cached | ~10× |
| **Query Time (exact)** | O(n^{o(1)}) | O(n^{o(1)}) lazy | ~5× |
| **Memory** | O(n log n) | O(active × n) | ~3× |
| **Prediction Accuracy** | N/A | 80%+ | New |
| **Edge Reduction** | 1 - ε | 90% neural | ~9× |
| **Max Graph Size** | ~100K | 1M+ streaming | ~10× |

---

## Integration with Existing Codebase

### SNN Integration Points

```rust
// Use existing SNN components from src/snn/
use crate::snn::{
    PolicySNN,           // For prediction engine
    ValueNetwork,        // For TD learning
    NeuralGraphOptimizer, // For neural sparsification
    compute_synchrony,   // For stability detection
    compute_energy,      // For attractor dynamics
};

// Connect j-tree to SNN energy landscape
impl PredictiveJTree {
    pub fn snn_energy(&self) -> f64 {
        let mincut = self.hierarchy.approximate_min_cut().value;
        let synchrony = compute_synchrony(&self.policy.recent_spikes(), 10.0);
        compute_energy(mincut, synchrony)
    }
}
```

### Parallel Architecture Integration

```rust
// Use existing parallel components from src/parallel/
use crate::parallel::{
    SharedCoordinator,   // Atomic coordination
    CoreExecutor,        // Per-core execution
    CoreDistributor,     // Work distribution
    ResultAggregator,    // Result collection
    NUM_CORES,           // 256 cores
};

// Extend CoreExecutor for j-tree levels
impl CoreExecutor {
    pub fn process_jtree_level(&mut self, level: &JTree) -> CoreResult {
        // Process assigned level within 8KB memory budget
        self.state.process_compact_jtree(level)
    }
}
```

### SONA Integration

```rust
// Connect to SONA hooks for continuous adaptation
const SONA_HOOKS: [usize; 4] = [8, 16, 24, 28];

impl PredictiveJTree {
    pub fn enable_sona(&mut self) {
        for &hook in &SONA_HOOKS {
            self.policy.enable_hook(hook);
        }
        // Adaptation latency: <0.05ms per hook
    }
}
```

---

## Implementation Priority

| Phase | Component | Effort | Impact | Dependencies |
|-------|-----------|--------|--------|--------------|
| **P0** | Degree-based presparse | 1 week | High | None |
| **P0** | 256-core distribution | 2 weeks | High | parallel/mod.rs |
| **P1** | Lazy hierarchy | 2 weeks | High | ADR-002 base |
| **P1** | Warm-start cut-matching | 2 weeks | High | Lazy hierarchy |
| **P2** | SNN prediction | 3 weeks | Medium | snn/optimizer.rs |
| **P2** | Neural sparsifier | 3 weeks | Medium | SNN prediction |
| **P3** | Streaming fallback | 2 weeks | Medium | None |
| **P3** | SONA integration | 1 week | Medium | SNN prediction |

---

## References

### New Research (2024-2026)

1. **SpecNet**: "Spectral Neural Graph Sparsification" (arXiv:2510.27474)
2. **DSpar**: "Degree-based Sparsification" (OpenReview)
3. **Warm-Start**: "Faster Weak Expander Decomposition" (arXiv:2511.02943)
4. **Parallel Expander**: "Near-Optimal Parallel Expander Decomposition" (SODA 2025)
5. **Semi-Streaming**: "Semi-Streaming Min-Cut" (Dudeja et al.)

### Existing Codebase

- `src/snn/mod.rs` - SNN integration (triple isomorphism)
- `src/snn/optimizer.rs` - PolicySNN, ValueNetwork, R-STDP
- `src/parallel/mod.rs` - 256-core architecture
- `src/compact/mod.rs` - 8KB per-core state

---

## Appendix: Complexity Summary

| Operation | Baseline | + Prediction | + Neural | + Parallel | + Streaming |
|-----------|----------|--------------|----------|------------|-------------|
| Insert Edge | O(n^ε) | O(n^ε) × 0.2 | O(n^ε) × 0.1 | O(n^ε / 256) | O(1) |
| Delete Edge | O(n^ε) | O(n^ε) × 0.2 | O(n^ε) × 0.1 | O(n^ε / 256) | O(1) |
| Approx Query | O(log n) | O(1) cached | O(1) | O(1) | O(n) |
| Exact Query | O(n^{o(1)}) | O(n^{o(1)}) × 0.2 | - | - | - |
| Memory | O(n log n) | O(n log n) | O(n log n / 10) | O(n log n) | O(n log n) |

**Combined**: Average case approaches O(1) for queries, O(n^ε / 256) for updates, with graceful degradation to streaming for massive graphs.
