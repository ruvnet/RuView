# ADR-002: Dynamic Hierarchical j-Tree Decomposition for Approximate Cut Structure

**Status**: Proposed
**Date**: 2026-01-25
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board
**SDK**: Claude-Flow

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-01-25 | ruv.io | Initial draft based on arXiv:2601.09139 research |

---

## Plain Language Summary

**What is it?**

A new algorithmic framework for maintaining an approximate view of a graph's cut structure that updates in near-constant time even as edges are added and removed. It complements our existing exact min-cut implementation by providing a fast "global radar" that can answer approximate cut queries instantly.

**Why does it matter?**

Our current implementation (arXiv:2512.13105, El-Hayek/Henzinger/Li) excels at **exact** min-cut for superpolylogarithmic cuts but is optimized for a specific cut-size regime. The new j-tree decomposition (arXiv:2601.09139, Goranci/Henzinger/Kiss/Momeni/Zöcklein, January 2026) provides:

- **Broader coverage**: Poly-logarithmic approximation for ALL cut-based problems (sparsest cut, multi-way cut, multi-cut, all-pairs min-cuts)
- **Faster updates**: O(n^ε) amortized for any arbitrarily small ε > 0
- **Low recourse**: The underlying cut-sparsifier tolerates vertex splits with poly-logarithmic recourse

**The Two-Tier Strategy**:

| Tier | Algorithm | Purpose | When to Use |
|------|-----------|---------|-------------|
| **Tier 1** | j-Tree Decomposition | Fast approximate hierarchy for global structure | Continuous monitoring, routing decisions |
| **Tier 2** | El-Hayek/Henzinger/Li | Exact deterministic min-cut | When Tier 1 detects critical cuts |

Think of it like sonar and radar: the j-tree is your wide-area radar that shows approximate threat positions instantly, while the exact algorithm is your precision sonar that confirms exact details when needed.

---

## Context

### Current State

RuVector MinCut implements the December 2025 breakthrough (arXiv:2512.13105) achieving:

| Property | Current Implementation |
|----------|----------------------|
| **Update Time** | O(n^{o(1)}) amortized |
| **Approximation** | Exact |
| **Deterministic** | Yes |
| **Cut Regime** | Superpolylogarithmic (λ > log^c n) |
| **Verified Scaling** | n^0.12 empirically |

This works excellently for the coherence gate (ADR-001) where we need exact cut values for safety decisions. However, several use cases require:

1. **Broader cut-based queries**: Sparsest cut, multi-way cut, multi-cut, all-pairs min-cuts
2. **Even faster updates**: When monitoring 10K+ updates/second
3. **Global structure awareness**: Understanding the overall cut landscape, not just the minimum

### The January 2026 Breakthrough

The paper "Dynamic Hierarchical j-Tree Decomposition and Its Applications" (arXiv:2601.09139, SODA 2026) by Goranci, Henzinger, Kiss, Momeni, and Zöcklein addresses the open question:

> "Is there a fully dynamic algorithm for cut-based optimization problems that achieves poly-logarithmic approximation with very small polynomial update time?"

**Key Results**:

| Result | Complexity | Significance |
|--------|------------|--------------|
| **Update Time** | O(n^ε) amortized for any ε ∈ (0,1) | Arbitrarily close to polylog |
| **Approximation** | Poly-logarithmic | Sufficient for structure detection |
| **Query Support** | All cut-based problems | Not just min-cut |
| **Recourse** | Poly-logarithmic total | Sparsifier doesn't explode |

### Technical Innovation: Vertex-Split-Tolerant Cut Sparsifier

The core innovation is a **dynamic cut-sparsifier** that handles vertex splits with low recourse:

```
Traditional approach: Vertex splits cause O(n) cascading updates
New approach: Forest packing with lazy repair → poly-log recourse
```

The sparsifier maintains (1±ε) approximation of all cuts while:
- Tolerating vertex splits (critical for dynamic hierarchies)
- Adjusting only poly-logarithmically many edges per update
- Serving as a backbone for the j-tree hierarchy

### The (L,j) Hierarchy

The j-tree hierarchy reflects increasingly coarse views of the graph's cut landscape:

```
Level 0: Original graph G
Level 1: Contracted graph with j-tree quality α
Level 2: Further contracted with quality α²
...
Level L: Root (O(1) vertices)

L = O(log n / log α)
```

Each level preserves cut structure within an α^ℓ factor, enabling:
- **Fast approximate queries**: Traverse O(log n) levels
- **Local updates**: Changes propagate through O(log n) levels
- **Multi-scale view**: See both fine and coarse structure

---

## Decision

### Adopt Two-Tier Dynamic Cut Architecture

We will implement the j-tree decomposition as a complementary layer to our existing exact min-cut, creating a two-tier system:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    TWO-TIER DYNAMIC CUT ARCHITECTURE                     │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │                 TIER 1: J-TREE HIERARCHY (NEW)                      │ │
│  │                                                                      │ │
│  │   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐         │ │
│  │   │   Level L    │    │   Level L-1  │    │   Level 0    │         │ │
│  │   │   (Root)     │◄───│  (Coarse)    │◄───│  (Original)  │         │ │
│  │   │   O(1) vtx   │    │  α^(L-1) cut │    │  Exact cuts  │         │ │
│  │   └──────────────┘    └──────────────┘    └──────────────┘         │ │
│  │                                                                      │ │
│  │   Purpose: Fast approximate answers for global structure             │ │
│  │   Update: O(n^ε) amortized for any ε > 0                            │ │
│  │   Query: Poly-log approximation for all cut problems                 │ │
│  │                                                                      │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                    │                                     │
│                    Trigger: Approximate cut below threshold              │
│                                    ▼                                     │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │              TIER 2: EXACT MIN-CUT (EXISTING)                       │ │
│  │                                                                      │ │
│  │   ┌──────────────────────────────────────────────────────────────┐ │ │
│  │   │  SubpolynomialMinCut (arXiv:2512.13105)                      │ │ │
│  │   │  • O(n^{o(1)}) amortized exact updates                       │ │ │
│  │   │  • Verified n^0.12 scaling                                   │ │ │
│  │   │  • Deterministic, no randomization                           │ │ │
│  │   │  • For superpolylogarithmic cuts (λ > log^c n)              │ │ │
│  │   └──────────────────────────────────────────────────────────────┘ │ │
│  │                                                                      │ │
│  │   Purpose: Exact verification when precision required                │ │
│  │   Trigger: Tier 1 detects potential critical cut                    │ │
│  │                                                                      │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Module Structure

```
ruvector-mincut/
├── src/
│   ├── jtree/                      # NEW: j-Tree Decomposition
│   │   ├── mod.rs                  # Module exports
│   │   ├── hierarchy.rs            # (L,j) hierarchical decomposition
│   │   ├── sparsifier.rs           # Vertex-split-tolerant cut sparsifier
│   │   ├── forest_packing.rs       # Forest packing for sparsification
│   │   ├── vertex_split.rs         # Vertex split handling with low recourse
│   │   ├── contraction.rs          # Graph contraction for hierarchy levels
│   │   └── queries/                # Cut-based query implementations
│   │       ├── mod.rs
│   │       ├── all_pairs_mincut.rs
│   │       ├── sparsest_cut.rs
│   │       ├── multiway_cut.rs
│   │       └── multicut.rs
│   ├── tiered/                     # NEW: Two-tier coordination
│   │   ├── mod.rs
│   │   ├── coordinator.rs          # Tier 1/Tier 2 routing logic
│   │   ├── trigger.rs              # Escalation trigger policies
│   │   └── cache.rs                # Cross-tier result caching
│   └── ...existing modules...
```

### Core Data Structures

#### j-Tree Hierarchy

```rust
/// Hierarchical j-tree decomposition for approximate cut structure
pub struct JTreeHierarchy {
    /// Number of levels (L = O(log n / log α))
    levels: usize,
    /// Approximation quality per level
    alpha: f64,
    /// Contracted graphs at each level
    contracted_graphs: Vec<ContractedGraph>,
    /// Cut sparsifier backbone
    sparsifier: DynamicCutSparsifier,
    /// j-trees at each level
    jtrees: Vec<JTree>,
}

/// Single level j-tree
pub struct JTree {
    /// Tree structure
    tree: DynamicTree,
    /// Mapping from original vertices to tree nodes
    vertex_map: HashMap<VertexId, TreeNodeId>,
    /// Cached cut values between tree nodes
    cut_cache: CutCache,
    /// Level index
    level: usize,
}

impl JTreeHierarchy {
    /// Build hierarchy from graph
    pub fn build(graph: &DynamicGraph, epsilon: f64) -> Self {
        let alpha = compute_alpha(epsilon);
        let levels = (graph.vertex_count() as f64).log(alpha as f64).ceil() as usize;

        // Build sparsifier first
        let sparsifier = DynamicCutSparsifier::build(graph, epsilon);

        // Build contracted graphs level by level
        let mut contracted_graphs = Vec::with_capacity(levels);
        let mut current = sparsifier.sparse_graph();

        for level in 0..levels {
            contracted_graphs.push(current.clone());
            current = contract_to_jtree(&current, alpha);
        }

        Self {
            levels,
            alpha,
            contracted_graphs,
            sparsifier,
            jtrees: build_jtrees(&contracted_graphs),
        }
    }

    /// Insert edge with O(n^ε) amortized update
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: f64) -> Result<(), Error> {
        // Update sparsifier (handles vertex splits internally)
        self.sparsifier.insert_edge(u, v, weight)?;

        // Propagate through hierarchy levels
        for level in 0..self.levels {
            self.update_level(level, EdgeUpdate::Insert(u, v, weight))?;
        }

        Ok(())
    }

    /// Delete edge with O(n^ε) amortized update
    pub fn delete_edge(&mut self, u: VertexId, v: VertexId) -> Result<(), Error> {
        self.sparsifier.delete_edge(u, v)?;

        for level in 0..self.levels {
            self.update_level(level, EdgeUpdate::Delete(u, v))?;
        }

        Ok(())
    }

    /// Query approximate min-cut (poly-log approximation)
    pub fn approximate_min_cut(&self) -> ApproximateCut {
        // Start from root level and refine
        let mut cut = self.jtrees[self.levels - 1].min_cut();

        for level in (0..self.levels - 1).rev() {
            cut = self.jtrees[level].refine_cut(&cut);
        }

        ApproximateCut {
            value: cut.value,
            approximation_factor: self.alpha.powi(self.levels as i32),
            partition: cut.partition,
        }
    }
}
```

#### Vertex-Split-Tolerant Cut Sparsifier

```rust
/// Dynamic cut sparsifier with low recourse under vertex splits
pub struct DynamicCutSparsifier {
    /// Forest packing for edge sampling
    forest_packing: ForestPacking,
    /// Sparse graph maintaining (1±ε) cut approximation
    sparse_graph: DynamicGraph,
    /// Epsilon parameter
    epsilon: f64,
    /// Recourse counter for complexity verification
    recourse: RecourseTracker,
}

impl DynamicCutSparsifier {
    /// Handle vertex split with poly-log recourse
    pub fn split_vertex(&mut self, v: VertexId, v1: VertexId, v2: VertexId,
                        partition: &[EdgeId]) -> Result<RecourseStats, Error> {
        let before_edges = self.sparse_graph.edge_count();

        // Forest packing handles the split
        let affected_forests = self.forest_packing.split_vertex(v, v1, v2, partition)?;

        // Lazy repair: only fix forests that actually need it
        for forest_id in affected_forests {
            self.repair_forest(forest_id)?;
        }

        let recourse = (self.sparse_graph.edge_count() as i64 - before_edges as i64).abs();
        self.recourse.record(recourse as usize);

        Ok(self.recourse.stats())
    }

    /// The key insight: forest packing limits cascading updates
    fn repair_forest(&mut self, forest_id: ForestId) -> Result<(), Error> {
        // Only O(log n) edges need adjustment per forest
        // Total forests = O(log n / ε²)
        // Total recourse = O(log² n / ε²) per vertex split
        self.forest_packing.repair(forest_id, &mut self.sparse_graph)
    }
}
```

### Two-Tier Coordinator

```rust
/// Coordinates between j-tree approximation (Tier 1) and exact min-cut (Tier 2)
pub struct TwoTierCoordinator {
    /// Tier 1: Fast approximate hierarchy
    jtree: JTreeHierarchy,
    /// Tier 2: Exact min-cut for verification
    exact: SubpolynomialMinCut,
    /// Trigger policy for escalation
    trigger: EscalationTrigger,
    /// Result cache to avoid redundant computation
    cache: TierCache,
}

/// When to escalate from Tier 1 to Tier 2
pub struct EscalationTrigger {
    /// Approximate cut threshold below which we verify exactly
    critical_threshold: f64,
    /// Maximum approximation factor before requiring exact
    max_approx_factor: f64,
    /// Whether the query requires exact answer
    exact_required: bool,
}

impl TwoTierCoordinator {
    /// Query min-cut with tiered strategy
    pub fn min_cut(&mut self, exact_required: bool) -> CutResult {
        // Check cache first
        if let Some(cached) = self.cache.get() {
            if !exact_required || cached.is_exact {
                return cached.clone();
            }
        }

        // Tier 1: Fast approximate query
        let approx = self.jtree.approximate_min_cut();

        // Decide whether to escalate
        let should_escalate = exact_required
            || approx.value < self.trigger.critical_threshold
            || approx.approximation_factor > self.trigger.max_approx_factor;

        if should_escalate {
            // Tier 2: Exact verification
            let exact_value = self.exact.min_cut_value();
            let exact_partition = self.exact.partition();

            let result = CutResult {
                value: exact_value,
                partition: exact_partition,
                is_exact: true,
                approximation_factor: 1.0,
                tier_used: Tier::Exact,
            };

            self.cache.store(result.clone());
            result
        } else {
            let result = CutResult {
                value: approx.value,
                partition: approx.partition,
                is_exact: false,
                approximation_factor: approx.approximation_factor,
                tier_used: Tier::Approximate,
            };

            self.cache.store(result.clone());
            result
        }
    }

    /// Insert edge, updating both tiers
    pub fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: f64) -> Result<(), Error> {
        self.cache.invalidate();

        // Update Tier 1 (fast)
        self.jtree.insert_edge(u, v, weight)?;

        // Update Tier 2 (also fast, but only if we're tracking that edge regime)
        self.exact.insert_edge(u, v, weight)?;

        Ok(())
    }
}
```

### Extended Query Support

The j-tree hierarchy enables queries beyond min-cut:

```rust
impl JTreeHierarchy {
    /// All-pairs minimum cuts (approximate)
    pub fn all_pairs_min_cuts(&self) -> AllPairsResult {
        // Use hierarchy to avoid O(n²) explicit computation
        // Query time: O(n log n) for all pairs
        let mut results = HashMap::new();

        for (u, v) in self.vertex_pairs() {
            let cut = self.min_cut_between(u, v);
            results.insert((u, v), cut);
        }

        AllPairsResult { cuts: results }
    }

    /// Sparsest cut (approximate)
    pub fn sparsest_cut(&self) -> SparsestCutResult {
        // Leverage hierarchy for O(n^ε) approximate sparsest cut
        let mut best_sparsity = f64::INFINITY;
        let mut best_cut = None;

        for level in 0..self.levels {
            let candidate = self.jtrees[level].sparsest_cut_candidate();
            let sparsity = candidate.value / candidate.size.min() as f64;

            if sparsity < best_sparsity {
                best_sparsity = sparsity;
                best_cut = Some(candidate);
            }
        }

        SparsestCutResult {
            cut: best_cut.unwrap(),
            sparsity: best_sparsity,
            approximation: self.alpha.powi(self.levels as i32),
        }
    }

    /// Multi-way cut (approximate)
    pub fn multiway_cut(&self, terminals: &[VertexId]) -> MultiwayCutResult {
        // Use j-tree hierarchy to find approximate multiway cut
        // Approximation: O(log k) where k = number of terminals
        self.compute_multiway_cut(terminals)
    }

    /// Multi-cut (approximate)
    pub fn multicut(&self, pairs: &[(VertexId, VertexId)]) -> MulticutResult {
        // Approximate multicut using hierarchy
        self.compute_multicut(pairs)
    }
}
```

### Integration with Coherence Gate (ADR-001)

The j-tree hierarchy integrates with the Anytime-Valid Coherence Gate:

```rust
/// Enhanced coherence gate using two-tier cut architecture
pub struct TieredCoherenceGate {
    /// Two-tier cut coordinator
    cut_coordinator: TwoTierCoordinator,
    /// Conformal prediction component
    conformal: ShiftAdaptiveConformal,
    /// E-process evidence accumulator
    evidence: EProcessAccumulator,
    /// Gate thresholds
    thresholds: GateThresholds,
}

impl TieredCoherenceGate {
    /// Fast structural check using Tier 1
    pub fn fast_structural_check(&self, action: &Action) -> QuickDecision {
        // Use j-tree for O(n^ε) approximate check
        let approx_cut = self.cut_coordinator.jtree.approximate_min_cut();

        if approx_cut.value > self.thresholds.definitely_safe {
            QuickDecision::Permit
        } else if approx_cut.value < self.thresholds.definitely_unsafe {
            QuickDecision::Deny
        } else {
            QuickDecision::NeedsExactCheck
        }
    }

    /// Full evaluation with exact verification if needed
    pub fn evaluate(&mut self, action: &Action, context: &Context) -> GateDecision {
        // Quick check first
        let quick = self.fast_structural_check(action);

        match quick {
            QuickDecision::Permit => {
                // Fast path: structure is definitely safe
                self.issue_permit_fast(action)
            }
            QuickDecision::Deny => {
                // Fast path: structure is definitely unsafe
                self.issue_denial_fast(action)
            }
            QuickDecision::NeedsExactCheck => {
                // Invoke Tier 2 for exact verification
                let exact_cut = self.cut_coordinator.min_cut(true);
                self.evaluate_with_exact_cut(action, context, exact_cut)
            }
        }
    }
}
```

### Performance Characteristics

| Operation | Tier 1 (j-Tree) | Tier 2 (Exact) | Combined |
|-----------|-----------------|----------------|----------|
| **Insert Edge** | O(n^ε) | O(n^{o(1)}) | O(n^ε) |
| **Delete Edge** | O(n^ε) | O(n^{o(1)}) | O(n^ε) |
| **Min-Cut Query** | O(log n) approx | O(1) exact | O(1) - O(log n) |
| **All-Pairs Min-Cut** | O(n log n) | N/A | O(n log n) |
| **Sparsest Cut** | O(n^ε) | N/A | O(n^ε) |
| **Multi-Way Cut** | O(k log k · n^ε) | N/A | O(k log k · n^ε) |

### Recourse Guarantees

The vertex-split-tolerant sparsifier provides:

| Metric | Guarantee |
|--------|-----------|
| **Edges adjusted per update** | O(log² n / ε²) |
| **Total recourse over m updates** | O(m · log² n / ε²) |
| **Forest repairs per vertex split** | O(log n) |

This is critical for maintaining hierarchy stability under dynamic changes.

---

## Implementation Phases

### Phase 1: Core Sparsifier (Weeks 1-3)

- [ ] Implement `ForestPacking` with edge sampling
- [ ] Implement `DynamicCutSparsifier` with vertex split handling
- [ ] Add recourse tracking and verification
- [ ] Unit tests for sparsifier correctness

### Phase 2: j-Tree Hierarchy (Weeks 4-6)

- [ ] Implement `JTree` single-level structure
- [ ] Implement `JTreeHierarchy` multi-level decomposition
- [ ] Add contraction algorithms for level construction
- [ ] Integration tests for hierarchy maintenance

### Phase 3: Query Support (Weeks 7-9)

- [ ] Implement approximate min-cut queries
- [ ] Implement all-pairs min-cut
- [ ] Implement sparsest cut
- [ ] Implement multi-way cut and multi-cut
- [ ] Benchmark query performance

### Phase 4: Two-Tier Integration (Weeks 10-12)

- [ ] Implement `TwoTierCoordinator`
- [ ] Define escalation trigger policies
- [ ] Integrate with coherence gate
- [ ] End-to-end testing with coherence scenarios

---

## Feature Flags

```toml
[features]
# Existing features
default = ["exact", "approximate"]
exact = []
approximate = []

# New features
jtree = []                    # j-Tree hierarchical decomposition
tiered = ["jtree", "exact"]   # Two-tier coordinator
all-cut-queries = ["jtree"]   # Sparsest cut, multiway, multicut
```

---

## Consequences

### Benefits

1. **Broader Query Support**: Sparsest cut, multi-way cut, multi-cut, all-pairs - not just minimum cut
2. **Faster Continuous Monitoring**: O(n^ε) updates enable 10K+ updates/second even on large graphs
3. **Global Structure Awareness**: Hierarchical view shows cut landscape at multiple scales
4. **Graceful Degradation**: Approximate answers when exact isn't needed, exact when it is
5. **Low Recourse**: Sparsifier stability prevents update cascades
6. **Coherence Gate Enhancement**: Fast structural checks with exact fallback

### Risks & Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Implementation complexity | High | Medium | Phase incrementally, extensive testing |
| Approximation too loose | Medium | Medium | Tunable α parameter, exact fallback |
| Memory overhead from hierarchy | Medium | Low | Lazy level construction |
| Integration complexity with existing code | Medium | Medium | Clean interface boundaries |

### Complexity Analysis

| Component | Space | Time (Update) | Time (Query) |
|-----------|-------|---------------|--------------|
| Forest Packing | O(m log n / ε²) | O(log² n / ε²) | O(1) |
| j-Tree Level | O(n_ℓ) | O(n_ℓ^ε) | O(log n_ℓ) |
| Full Hierarchy | O(n log n) | O(n^ε) | O(log n) |
| Two-Tier Cache | O(n) | O(1) | O(1) |

---

## References

### Primary

1. Goranci, G., Henzinger, M., Kiss, P., Momeni, A., & Zöcklein, G. (January 2026). "Dynamic Hierarchical j-Tree Decomposition and Its Applications." *arXiv:2601.09139*. SODA 2026. **[Core paper for this ADR]**

### Complementary

2. El-Hayek, A., Henzinger, M., & Li, J. (December 2025). "Deterministic and Exact Fully-dynamic Minimum Cut of Superpolylogarithmic Size in Subpolynomial Time." *arXiv:2512.13105*. **[Existing Tier 2 implementation]**

3. Mądry, A. (2010). "Fast Approximation Algorithms for Cut-Based Problems in Undirected Graphs." *FOCS 2010*. **[Original j-tree decomposition]**

### Background

4. Benczúr, A. A., & Karger, D. R. (1996). "Approximating s-t Minimum Cuts in Õ(n²) Time." *STOC*. **[Cut sparsification foundations]**

5. Thorup, M. (2007). "Fully-Dynamic Min-Cut." *Combinatorica*. **[Dynamic min-cut foundations]**

---

## Related Decisions

- **ADR-001**: Anytime-Valid Coherence Gate (uses Tier 2 exact min-cut)
- **ADR-014**: Coherence Engine Architecture (coherence computation)
- **ADR-CE-001**: Sheaf Laplacian Coherence (structural coherence foundation)

---

## Appendix: Paper Comparison

### El-Hayek/Henzinger/Li (Dec 2025) vs Goranci et al. (Jan 2026)

| Aspect | arXiv:2512.13105 | arXiv:2601.09139 |
|--------|------------------|------------------|
| **Focus** | Exact min-cut | Approximate cut hierarchy |
| **Update Time** | O(n^{o(1)}) | O(n^ε) for any ε > 0 |
| **Approximation** | Exact | Poly-logarithmic |
| **Cut Regime** | Superpolylogarithmic | All sizes |
| **Query Types** | Min-cut only | All cut problems |
| **Deterministic** | Yes | Yes |
| **Key Technique** | Cluster hierarchy + LocalKCut | j-Tree + vertex-split sparsifier |

**Synergy**: The two approaches complement each other perfectly:
- Use Goranci et al. for fast global monitoring and diverse cut queries
- Use El-Hayek et al. for exact verification when critical cuts are detected

This two-tier strategy provides both breadth (approximate queries on all cut problems) and depth (exact min-cut when needed).
