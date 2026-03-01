# Causal Attention Networks (CAN)

## Overview

### Problem Statement
Standard attention mechanisms in GNNs ignore temporal and causal ordering, allowing future information to influence past states. This creates three critical issues:
1. **Information Leakage**: Future documents can influence retrieval of past documents
2. **Invalid Counterfactuals**: Cannot answer "what if this event never occurred?"
3. **Temporal Inconsistency**: Legal citations, event logs, and versioned documents require strict causal ordering

### Proposed Solution
Implement causal attention that respects temporal ordering through:
- Directed acyclic graph (DAG) structure enforcing causality
- Masked attention preventing future→past information flow
- Counterfactual query engine for "what-if" analysis
- Temporal consistency guarantees for ordered data

### Expected Benefits
- **100% prevention** of temporal information leakage
- **Counterfactual queries**: Answer "what if X didn't exist?" questions
- **Legal compliance**: Proper citation precedence in legal documents
- **Event causality**: Correct cause-effect relationships in logs
- **Version control**: Proper document evolution tracking

### Novelty Claim
First integration of strict causal inference principles into vector search. Unlike temporal embeddings (which encode time but don't enforce causality) or recurrent models (which only process sequences), CAN provides:
- Formal causal guarantees via DAG structure
- Counterfactual reasoning via intervention calculus
- Bi-directional queries (forward: "what did this cause?" backward: "what caused this?")

## Technical Design

### Architecture Diagram
```
┌──────────────────────────────────────────────────────────────────┐
│                    Causal Attention Network                       │
│                                                                    │
│  ┌────────────────────────────────────────────────────────┐     │
│  │                  Causal DAG Layer                       │     │
│  │                                                         │     │
│  │   t₀        t₁        t₂        t₃        t₄          │     │
│  │   ●────────▶●────────▶●────────▶●────────▶●           │     │
│  │   │         │╲        │╲        │         │           │     │
│  │   │         │ ╲       │ ╲       │         │           │     │
│  │   │         │  ╲      │  ╲      │         │           │     │
│  │   │         ▼   ╲     ▼   ╲     ▼         ▼           │     │
│  │   │         ●    └───▶●    └───▶●────────▶●           │     │
│  │   │         │         │         │         │           │     │
│  │   └────────▶●────────▶●────────▶●────────▶●           │     │
│  │                                                         │     │
│  │   Legend: ● = Node with timestamp                      │     │
│  │          ──▶ = Causal edge (past → future)            │     │
│  └────────────────────────────────────────────────────────┘     │
│                            │                                     │
│                            ▼                                     │
│  ┌────────────────────────────────────────────────────────┐     │
│  │              Masked Attention Matrix                    │     │
│  │                                                         │     │
│  │        q₀   q₁   q₂   q₃   q₄                         │     │
│  │   k₀ [ 1.0  0.0  0.0  0.0  0.0 ]  ◄─ No future info   │     │
│  │   k₁ [ 0.7  1.0  0.0  0.0  0.0 ]                      │     │
│  │   k₂ [ 0.4  0.6  1.0  0.0  0.0 ]                      │     │
│  │   k₃ [ 0.2  0.3  0.5  1.0  0.0 ]                      │     │
│  │   k₄ [ 0.1  0.2  0.3  0.6  1.0 ]                      │     │
│  │        ▲                                                │     │
│  │        └─ Upper triangle masked (set to -∞)            │     │
│  └────────────────────────────────────────────────────────┘     │
│                            │                                     │
│                            ▼                                     │
│  ┌────────────────────────────────────────────────────────┐     │
│  │           Counterfactual Query Engine                   │     │
│  │                                                         │     │
│  │  Query: "Results if document D₂ never existed?"        │     │
│  │                                                         │     │
│  │  1. Identify intervention: do(remove D₂)               │     │
│  │  2. Propagate intervention through DAG                 │     │
│  │  3. Recompute attention without D₂'s influence         │     │
│  │  4. Compare: Actual vs Counterfactual results          │     │
│  │                                                         │     │
│  └────────────────────────────────────────────────────────┘     │
└──────────────────────────────────────────────────────────────────┘
```

### Core Data Structures

```rust
/// Causal graph structure (DAG)
#[derive(Clone, Debug)]
pub struct CausalGraph {
    /// Nodes with temporal ordering
    nodes: Vec<CausalNode>,

    /// Adjacency list (only forward edges: past → future)
    edges: Vec<Vec<EdgeId>>,

    /// Topological ordering cache
    topo_order: Vec<NodeId>,

    /// Temporal index for fast time-based queries
    temporal_index: BTreeMap<Timestamp, Vec<NodeId>>,

    /// Reverse index (for backward causal queries)
    reverse_edges: Vec<Vec<EdgeId>>,
}

/// Node with causal metadata
#[derive(Clone, Debug)]
pub struct CausalNode {
    /// Unique identifier
    pub id: NodeId,

    /// Embedding vector
    pub embedding: Vec<f32>,

    /// Timestamp (must be monotonic)
    pub timestamp: Timestamp,

    /// Causal parents (nodes that influenced this one)
    pub parents: Vec<NodeId>,

    /// Causal children (nodes influenced by this one)
    pub children: Vec<NodeId>,

    /// Metadata (document type, version, etc.)
    pub metadata: HashMap<String, String>,
}

/// Timestamp with total ordering
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp {
    /// Seconds since epoch
    pub seconds: i64,

    /// Nanoseconds (for sub-second precision)
    pub nanos: u32,

    /// Logical clock (for events at same physical time)
    pub logical: u64,
}

/// Causal attention mask
#[derive(Clone, Debug)]
pub struct CausalMask {
    /// Sparse mask representation
    /// Only store allowed attention pairs
    allowed_pairs: HashSet<(NodeId, NodeId)>,

    /// Cached dense mask for small graphs
    dense_mask: Option<Array2<bool>>,

    /// Mask generation strategy
    strategy: MaskStrategy,
}

/// Mask generation strategies
#[derive(Clone, Debug)]
pub enum MaskStrategy {
    /// Strict: Only past nodes (timestamp < current)
    Strict,

    /// Window: Past N time units
    TimeWindow { duration: Duration },

    /// Topological: Follow DAG structure
    Topological { max_depth: usize },

    /// Custom predicate
    Custom(fn(&CausalNode, &CausalNode) -> bool),
}

/// Counterfactual intervention
#[derive(Clone, Debug)]
pub struct Intervention {
    /// Type of intervention
    pub kind: InterventionKind,

    /// Target nodes
    pub targets: Vec<NodeId>,

    /// Intervention strength (0.0 = no effect, 1.0 = complete removal)
    pub strength: f32,
}

#[derive(Clone, Debug)]
pub enum InterventionKind {
    /// Remove node entirely
    Remove,

    /// Set embedding to specific value
    SetValue(Vec<f32>),

    /// Block causal influence (cut edges)
    BlockInfluence,

    /// Add hypothetical node
    AddNode(CausalNode),
}

/// Counterfactual query result
#[derive(Clone, Debug)]
pub struct CounterfactualResult {
    /// Actual (factual) results
    pub factual: Vec<SearchResult>,

    /// Counterfactual results (with intervention)
    pub counterfactual: Vec<SearchResult>,

    /// Difference analysis
    pub differences: Vec<Difference>,

    /// Causal effect size
    pub effect_size: f32,
}

#[derive(Clone, Debug)]
pub struct Difference {
    pub node_id: NodeId,
    pub rank_change: i32,
    pub score_change: f32,
    pub explanation: String,
}
```

### Key Algorithms

```rust
// Pseudocode for causal attention

/// Build causal mask from temporal ordering
fn build_causal_mask(
    graph: &CausalGraph,
    strategy: MaskStrategy
) -> CausalMask {
    let mut allowed_pairs = HashSet::new();

    for node in &graph.nodes {
        match strategy {
            MaskStrategy::Strict => {
                // Allow attention only to earlier nodes
                for other in &graph.nodes {
                    if other.timestamp < node.timestamp {
                        allowed_pairs.insert((node.id, other.id));
                    }
                }
            },

            MaskStrategy::TimeWindow { duration } => {
                // Allow attention within time window
                let cutoff = node.timestamp - duration;
                for other in &graph.nodes {
                    if other.timestamp >= cutoff && other.timestamp < node.timestamp {
                        allowed_pairs.insert((node.id, other.id));
                    }
                }
            },

            MaskStrategy::Topological { max_depth } => {
                // Allow attention to ancestors in DAG
                let ancestors = find_ancestors(graph, node.id, max_depth);
                for ancestor in ancestors {
                    allowed_pairs.insert((node.id, ancestor));
                }
            },

            MaskStrategy::Custom(predicate) => {
                for other in &graph.nodes {
                    if predicate(node, other) {
                        allowed_pairs.insert((node.id, other.id));
                    }
                }
            },
        }
    }

    CausalMask {
        allowed_pairs,
        dense_mask: None,  // Lazily computed
        strategy,
    }
}

/// Causal attention computation
fn causal_attention(
    query: &[f32],
    graph: &CausalGraph,
    mask: &CausalMask,
    k: usize
) -> Vec<SearchResult> {
    let mut scores = Vec::new();

    // Compute attention scores
    for node in &graph.nodes {
        let score = cosine_similarity(query, &node.embedding);
        scores.push((node.id, score));
    }

    // Apply causal mask
    scores.retain(|(node_id, _)| {
        // For query at "current time", only attend to past
        let query_time = Timestamp::now();
        let node = &graph.nodes[*node_id];
        node.timestamp < query_time
    });

    // Sort by score and return top-k
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    scores.into_iter()
        .take(k)
        .map(|(id, score)| SearchResult { id, score })
        .collect()
}

/// Counterfactual query with intervention
fn counterfactual_query(
    query: &[f32],
    graph: &CausalGraph,
    intervention: &Intervention,
    k: usize
) -> CounterfactualResult {
    // Step 1: Compute factual results (no intervention)
    let factual = causal_attention(query, graph, &graph.default_mask, k);

    // Step 2: Apply intervention
    let mut modified_graph = graph.clone();
    apply_intervention(&mut modified_graph, intervention);

    // Step 3: Compute counterfactual results
    let counterfactual = causal_attention(
        query,
        &modified_graph,
        &modified_graph.default_mask,
        k
    );

    // Step 4: Analyze differences
    let differences = compute_differences(&factual, &counterfactual);

    // Step 5: Compute causal effect size
    let effect_size = compute_effect_size(&factual, &counterfactual);

    CounterfactualResult {
        factual,
        counterfactual,
        differences,
        effect_size,
    }
}

/// Apply intervention to graph
fn apply_intervention(
    graph: &mut CausalGraph,
    intervention: &Intervention
) {
    match &intervention.kind {
        InterventionKind::Remove => {
            // Remove nodes and their causal influence
            for target in &intervention.targets {
                // Mark node as removed
                graph.nodes[*target].metadata.insert(
                    "removed".to_string(),
                    "true".to_string()
                );

                // Cut all outgoing edges (prevent future influence)
                graph.edges[*target].clear();

                // Remove incoming edges (erase past influence)
                for parent in &graph.nodes[*target].parents.clone() {
                    graph.edges[*parent].retain(|e| {
                        graph.get_edge(*e).target != *target
                    });
                }
            }

            // Recompute topological order
            graph.recompute_topo_order();
        },

        InterventionKind::SetValue(new_embedding) => {
            // Change embedding value
            for target in &intervention.targets {
                graph.nodes[*target].embedding = new_embedding.clone();
            }
        },

        InterventionKind::BlockInfluence => {
            // Cut outgoing edges but keep node
            for target in &intervention.targets {
                graph.edges[*target].clear();
            }
        },

        InterventionKind::AddNode(new_node) => {
            // Add hypothetical node
            graph.add_node(new_node.clone());
            graph.recompute_topo_order();
        },
    }
}

/// Topological sort for DAG
fn topological_sort(graph: &CausalGraph) -> Vec<NodeId> {
    let mut in_degree = vec![0; graph.nodes.len()];

    // Compute in-degrees
    for edges in &graph.edges {
        for edge_id in edges {
            let target = graph.get_edge(*edge_id).target;
            in_degree[target] += 1;
        }
    }

    // Kahn's algorithm
    let mut queue: VecDeque<NodeId> = in_degree.iter()
        .enumerate()
        .filter(|(_, &deg)| deg == 0)
        .map(|(id, _)| id)
        .collect();

    let mut result = Vec::new();

    while let Some(node) = queue.pop_front() {
        result.push(node);

        for edge_id in &graph.edges[node] {
            let target = graph.get_edge(*edge_id).target;
            in_degree[target] -= 1;
            if in_degree[target] == 0 {
                queue.push_back(target);
            }
        }
    }

    assert_eq!(result.len(), graph.nodes.len(), "Graph has cycle!");
    result
}
```

### API Design

```rust
/// Public API for Causal Attention Networks
pub trait CausalAttention {
    /// Create causal graph from timestamped documents
    fn new(documents: Vec<Document>, config: CausalConfig) -> Self;

    /// Search with causal constraints
    fn search(
        &self,
        query: &[f32],
        k: usize,
        options: CausalSearchOptions,
    ) -> Result<Vec<SearchResult>, CanError>;

    /// Counterfactual query
    fn counterfactual(
        &self,
        query: &[f32],
        intervention: Intervention,
        k: usize,
    ) -> Result<CounterfactualResult, CanError>;

    /// Forward causal query: "What did X cause?"
    fn forward_causal(
        &self,
        source: NodeId,
        max_depth: usize,
    ) -> Result<Vec<NodeId>, CanError>;

    /// Backward causal query: "What caused X?"
    fn backward_causal(
        &self,
        target: NodeId,
        max_depth: usize,
    ) -> Result<Vec<NodeId>, CanError>;

    /// Add new node with temporal ordering
    fn add_node(&mut self, node: CausalNode) -> Result<NodeId, CanError>;

    /// Verify causal consistency
    fn verify_consistency(&self) -> Result<(), CanError>;

    /// Export causal graph for visualization
    fn export_graph(&self) -> CausalGraphExport;
}

/// Configuration for causal attention
#[derive(Clone, Debug)]
pub struct CausalConfig {
    /// Mask generation strategy
    pub mask_strategy: MaskStrategy,

    /// Allow concurrent events (same timestamp)?
    pub allow_concurrent: bool,

    /// Automatic edge inference from timestamps
    pub infer_edges: bool,

    /// Maximum causal depth for queries
    pub max_depth: usize,
}

/// Search options with causal constraints
#[derive(Clone, Debug)]
pub struct CausalSearchOptions {
    /// Search only before this timestamp
    pub before: Option<Timestamp>,

    /// Search only after this timestamp
    pub after: Option<Timestamp>,

    /// Require specific causal path
    pub require_path: Option<Vec<NodeId>>,

    /// Exclude nodes and their descendants
    pub exclude: Vec<NodeId>,
}

/// Causal graph export format
#[derive(Clone, Debug, Serialize)]
pub struct CausalGraphExport {
    pub nodes: Vec<ExportNode>,
    pub edges: Vec<ExportEdge>,
    pub metadata: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExportNode {
    pub id: NodeId,
    pub timestamp: Timestamp,
    pub label: String,
    pub position: (f32, f32),  // For visualization
}

#[derive(Clone, Debug, Serialize)]
pub struct ExportEdge {
    pub source: NodeId,
    pub target: NodeId,
    pub weight: f32,
}
```

## Integration Points

### Affected Crates/Modules

1. **`crates/ruvector-core/src/hnsw/`**
   - Extend to support directed edges (DAG structure)
   - Add temporal metadata to nodes
   - Modify search to respect causal constraints

2. **`crates/ruvector-gnn/src/attention/`**
   - Add causal masking to attention mechanisms
   - Integrate with existing attention variants

3. **`crates/ruvector-core/src/index/`**
   - Add temporal indexing for fast time-based queries
   - Support DAG-based navigation

### New Modules to Create

1. **`crates/ruvector-gnn/src/causal/`**
   - `graph.rs` - Causal DAG implementation
   - `mask.rs` - Causal masking strategies
   - `intervention.rs` - Counterfactual interventions
   - `search.rs` - Causal search algorithms
   - `verify.rs` - Consistency checking
   - `temporal.rs` - Timestamp and ordering utilities

2. **`crates/ruvector-core/src/temporal/`**
   - `index.rs` - Temporal indexing structures
   - `ordering.rs` - Total order on timestamps
   - `version.rs` - Document versioning support

### Dependencies on Other Features

- **Feature 10 (Gravitational Fields)**: GEF must respect causal ordering (no backward pull)
- **Feature 12 (Topology-Aware Routing)**: Topology metrics need DAG-aware computation
- **Feature 13 (Crystallization)**: Hierarchies must respect temporal precedence

## Regression Prevention

### Existing Functionality at Risk

1. **Undirected Graph Search**
   - Risk: Breaking existing HNSW bidirectional search
   - Prevention: Maintain separate directed/undirected graph modes

2. **Performance Overhead**
   - Risk: Topological sort and mask computation add latency
   - Prevention: Cache masks, lazy computation, optional feature

3. **Storage Overhead**
   - Risk: Timestamp + edge direction doubles metadata
   - Prevention: Optional temporal metadata, compressed timestamps

### Test Cases to Prevent Regressions

```rust
#[cfg(test)]
mod regression_tests {
    /// Verify no temporal leakage
    #[test]
    fn test_no_future_information() {
        let mut graph = CausalGraph::new(CausalConfig::default());

        // Add nodes with increasing timestamps
        let past = graph.add_node(node_at_time(t0));
        let present = graph.add_node(node_at_time(t1));
        let future = graph.add_node(node_at_time(t2));

        // Query from present: should not see future
        let results = graph.search(&query, 10, CausalSearchOptions {
            before: Some(t1),
            ..Default::default()
        });

        assert!(!results.contains(&future));
        assert!(results.contains(&past));
    }

    /// Counterfactual removal test
    #[test]
    fn test_counterfactual_removal() {
        let graph = create_legal_citation_graph();

        // Factual: Case A cites Case B
        let factual = graph.search(&case_a_query, 10);
        assert!(factual.contains(&case_b));

        // Counterfactual: What if Case B never existed?
        let intervention = Intervention {
            kind: InterventionKind::Remove,
            targets: vec![case_b],
            strength: 1.0,
        };

        let counterfactual = graph.counterfactual(
            &case_a_query,
            intervention,
            10
        );

        assert!(!counterfactual.counterfactual.contains(&case_b));
        assert_ne!(factual, counterfactual.factual);
    }

    /// DAG consistency
    #[test]
    fn test_dag_no_cycles() {
        let graph = create_random_causal_graph(1000);

        // Should not panic (cycle detection)
        let topo = graph.topological_sort();
        assert_eq!(topo.len(), 1000);

        // Verify all edges go forward in topological order
        for (source, edges) in graph.edges.iter().enumerate() {
            for edge in edges {
                let target = graph.get_edge(*edge).target;
                let source_pos = topo.iter().position(|&id| id == source).unwrap();
                let target_pos = topo.iter().position(|&id| id == target).unwrap();
                assert!(source_pos < target_pos, "Edge goes backward!");
            }
        }
    }
}
```

### Backward Compatibility Strategy

1. **Dual Mode**: Support both causal and non-causal graphs
2. **Automatic Detection**: Infer causality from timestamp metadata
3. **Migration Tool**: Convert existing graphs to causal structure
4. **Graceful Degradation**: If no timestamps, fall back to standard search

## Implementation Phases

### Phase 1: Research Validation (2 weeks)
**Goal**: Validate causal inference on real-world data

- Implement basic DAG structure and topological sort
- Create legal citation dataset with known causal structure
- Test counterfactual queries on synthetic data
- Measure temporal leakage prevention
- **Deliverable**: Research report with causal correctness proofs

### Phase 2: Core Implementation (3 weeks)
**Goal**: Production causal graph system

- Implement `CausalGraph` with temporal indexing
- Develop causal masking strategies
- Build intervention engine
- Add forward/backward causal queries
- Implement consistency verification
- **Deliverable**: Working CAN module with unit tests

### Phase 3: Integration (2 weeks)
**Goal**: Integrate with RuVector ecosystem

- Add temporal metadata to HNSW nodes
- Implement DAG serialization/deserialization
- Create API bindings (Python, Node.js)
- Add visualization tools (Graphviz export)
- Write integration tests
- **Deliverable**: CAN integrated into main codebase

### Phase 4: Optimization (2 weeks)
**Goal**: Production performance

- Profile and optimize topological sort
- Implement sparse mask representations
- Add incremental updates (streaming DAG)
- Create benchmarks for legal/event datasets
- Write documentation and examples
- **Deliverable**: Production-ready, documented feature

## Success Metrics

### Performance Benchmarks

| Metric | Baseline | Target | Measurement |
|--------|----------|--------|-------------|
| Temporal leakage rate | N/A | 0% | Verified by test suite |
| Causal query latency | N/A | <2ms | 99th percentile, 10K nodes |
| Counterfactual overhead | N/A | <5x | vs. standard search |
| Memory overhead | 0MB | <100MB | Per 1M nodes (timestamps+edges) |
| DAG update latency | N/A | <1ms | Add node with edge inference |

### Accuracy Metrics

1. **Temporal Correctness**: No future information in results
   - Target: 100% correctness (formal verification)

2. **Counterfactual Validity**: Interventions produce expected changes
   - Target: >95% agreement with manual counterfactual analysis

3. **Causal Path Accuracy**: Correct ancestor/descendant relationships
   - Target: 100% correctness on citation graphs

### Comparison to Baselines

Test against:
1. **Standard Attention**: Temporal leakage analysis
2. **Temporal Embeddings**: Counterfactual capability comparison
3. **RNNs/LSTMs**: Bi-directional causal query performance

Datasets:
- Legal citations (Caselaw Access Project, 6M cases)
- arXiv citations (2M papers with temporal metadata)
- Wikipedia edit history (versioned documents)
- Event logs (system logs, user actions)

## Risks and Mitigations

### Technical Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Cycle detection bugs | High | Low | Extensive testing, formal verification |
| Timestamp conflicts | Medium | Medium | Logical clocks, conflict resolution |
| Counterfactual explosion | High | Medium | Limit intervention scope, caching |
| DAG update complexity | Medium | High | Incremental algorithms, batching |
| Poor timestamp quality | High | High | Automatic inference, validation |

### Detailed Mitigations

1. **Cycle Detection Bugs**
   - Implement multiple cycle detection algorithms (DFS, Kahn's)
   - Property-based testing (QuickCheck)
   - Formal proof of DAG invariants
   - Fallback: Reject graphs with cycles

2. **Timestamp Conflicts**
   - Use hybrid logical clocks (HLC) for concurrent events
   - Implement timestamp resolution strategies
   - Allow manual timestamp assignment
   - Fallback: Use insertion order as logical time

3. **Counterfactual Explosion**
   - Limit intervention depth (max descendants affected)
   - Implement intervention caching
   - Use approximate counterfactuals for large graphs
   - Fallback: Disable counterfactuals for >1M nodes

4. **DAG Update Complexity**
   - Implement incremental topological sort (Pearce-Kelly)
   - Batch insertions for better amortized cost
   - Use lazy recomputation strategies
   - Fallback: Full recomputation only when needed

5. **Poor Timestamp Quality**
   - Infer timestamps from document metadata
   - Cross-reference multiple time sources
   - Implement timestamp validation heuristics
   - Fallback: Warn user and disable causal guarantees

## Applications

### Legal Document Search
- Citation precedence: Only cite earlier cases
- Counterfactual: "Would this case still apply if landmark case X was overturned?"
- Temporal queries: "Find cases before 2020 about patent law"

### Event Log Analysis
- Root cause analysis: "What caused this failure?"
- Impact analysis: "What did this configuration change affect?"
- Counterfactual: "What if we hadn't deployed version 2.3?"

### Version Control
- Document evolution: "Show me earlier versions of this section"
- Blame analysis: "Which change introduced this concept?"
- Counterfactual: "What would docs look like without the API redesign?"

### Knowledge Graphs
- Temporal reasoning: "What was known about X in 2015?"
- Causal inference: "Did discovery A enable discovery B?"
- Counterfactual: "What if theory X was never proposed?"

## References

### Causal Inference Theory
- Pearl's causality framework (do-calculus)
- Directed Acyclic Graphs (DAGs) for causality
- Counterfactual reasoning and interventions
- Granger causality for time series

### Temporal Modeling
- Temporal knowledge graphs
- Hybrid logical clocks (HLC)
- Version control theory (DAG of commits)
- Event sourcing and CQRS

### Implementation Techniques
- Incremental topological sorting
- Sparse attention masks
- Efficient DAG operations
- Temporal indexing structures
